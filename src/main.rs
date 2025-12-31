mod logger;
mod mysignal;
mod signal;
mod update;

use std::io::prelude::*;
use std::{collections::HashMap, fmt::Debug, fs::File, hash::Hash, sync::Arc, vec};

use presage::{
  libsignal_service::{
    Profile,
    configuration::SignalServers,
    prelude::{ProfileKey, Uuid},
    zkgroup::GroupMasterKeyBytes,
  },
  model::groups::Group,
  store::Thread,
};

use presage::manager::Manager;
use presage::model::messages::Received;
use presage::store::StateStore;
use presage_store_sqlite::{OnNewIdentity, SqliteStore};
// use crate::database::{OnNewIdentity, SqliteStore};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use url::Url;
// use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};

use qrcodegen::QrCode;
use qrcodegen::QrCodeEcc;
// use crate::signal::*;
use crate::signal::{Cmd, link_device};
use crate::signal::{default_db_path, list_groups};
use crate::update::*;
use crate::{logger::Logger, mysignal::SignalSpawner, update::LinkingAction};

/// This example connects to a radio via serial and prints out all received packets.
/// This example requires a powered and flashed Meshtastic radio.
/// https://meshtastic.org/docs/supported-hardware
// use std::io::{self, BufRead};
mod dumb_packet_router;
use dumb_packet_router::DumbPacketRouter;

use meshtastic::api::StreamApi;
use meshtastic::packet::PacketDestination;
use meshtastic::protobufs::{FromRadio, NodeInfo, User, mesh_packet};
use meshtastic::types::MeshChannel;
use meshtastic::utils;

// This import allows for decoding of mesh packets
// Re-export of prost::Message
use meshtastic::Message;
use meshtastic::protobufs;

type Nodes = HashMap<u32, meshtastic::protobufs::NodeInfo>;

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//   // println!("Received: {:?}", decoded);
//
//   // Note that in this specific example, this will only be called when
//   // the radio is disconnected, as the above loop will never exit.
//   // Typically, you would allow the user to manually kill the loop,
//   // for example, with tokio::select!.
//   let _stream_api = stream_api.disconnect().await?;
//
//   Ok(())
// }

// fn print_node_info(info: protobufs::NodeInfo) {}

/// A helper function to handle packets coming directly from the radio connection.
/// The Meshtastic `PhoneAPI` will return decoded `FromRadio` packets, which
/// can then be handled based on their payload variant. Note that the payload
/// variant can be `None`, in which case the packet should be ignored.
fn handle_from_radio_packet(
  from_radio_packet: meshtastic::protobufs::FromRadio,
  config: &Config,
  nodes: &mut Nodes,
) -> Option<Action> {
  // let cloned_packet = from_radio_packet.clone();
  // Remove `None` variants to get the payload variant
  let payload_variant = match from_radio_packet.payload_variant {
    Some(payload_variant) => payload_variant,
    None => {
      println!("Received FromRadio packet with no payload variant, not handling...");
      return None;
    }
  };

  // `FromRadio` packets can be differentiated based on their payload variant,
  // which in Rust is represented as an enum. This means the payload variant
  // can be matched on, and the appropriate user-defined action can be taken.
  match payload_variant {
    meshtastic::protobufs::from_radio::PayloadVariant::Channel(channel) => {
      println!("Received channel packet: {:?}", channel);
    }
    meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
      nodes.insert(node_info.num, node_info);
    }
    meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
      return handle_mesh_packet(mesh_packet, nodes, config);
    }
    _ => {
      // println!("Received other FromRadio packet, not handling...");
    }
  };

  None
}

/// A helper function to handle `MeshPacket` messages, which are a subset
/// of all `FromRadio` messages. Note that the payload variant can be `None`,
/// and that the payload variant can be `Encrypted`, in which case the packet
/// should be ignored within client applications.
///
/// Mesh packets are the most commonly used type of packet, and are usually
/// what people are referring to when they talk about "packets."
fn handle_mesh_packet(
  mesh_packet: protobufs::MeshPacket,
  nodes: &Nodes,
  config: &Config,
) -> Option<Action> {
  let cloned_packet = mesh_packet.clone();
  // Remove `None` variants to get the payload variant

  // Only handle decoded (unencrypted) mesh packets
  let packet_data = match mesh_packet.payload_variant {
    Some(protobufs::mesh_packet::PayloadVariant::Decoded(decoded_mesh_packet)) => decoded_mesh_packet,
    Some(protobufs::mesh_packet::PayloadVariant::Encrypted(_encrypted_mesh_packet)) => {
      // println!("Received encrypted mesh packet, not handling...");
      return None;
    }
    None => {
      println!("Received mesh packet with no payload variant, not handling...");
      return None;
    }
  };

  // Meshtastic differentiates mesh packets based on a field called `portnum`.
  // Meshtastic defines a set of standard port numbers [here](https://meshtastic.org/docs/development/firmware/portnum),
  // but also allows for custom port numbers to be used.
  match packet_data.portnum() {
    meshtastic::protobufs::PortNum::PositionApp => {
      // Note that `Data` structs contain a `payload` field, which is a vector of bytes.
      // This data needs to be decoded into a protobuf struct, which is shown below.
      // The `decode` function is provided by the `prost` crate, which is re-exported
      // by the `meshtastic` crate.
      let decoded_position =
        meshtastic::protobufs::Position::decode(packet_data.payload.as_slice()).unwrap();

      println!("Received position packet: {:?}", decoded_position);
    }

    meshtastic::protobufs::PortNum::TextMessageApp => match mesh_packet.channel {
      0 => {
        // println!("heres the whole packet: {:#?}", &cloned_packet);
        let decoded_text_message = String::from_utf8(packet_data.payload).unwrap();

        println!("Received DM message: {:?}", &decoded_text_message);

        if decoded_text_message == "/ping" {
          return Some(Action::SendToMesh {
            body: "pong!".to_string(),
            channel: 0.into(),
            destination: PacketDestination::Node(mesh_packet.from.into()),
          });
        }
      }
      1 => {
        // println!("heres the whole packet: {:#?}", &cloned_packet);
        let decoded_text_message = String::from_utf8(packet_data.payload).unwrap();

        println!("Received text message from channel: {:?}", &decoded_text_message);

        if decoded_text_message == "/ping" {
          return Some(Action::SendToMesh {
            body: "pong!".to_string(),
            channel: 1.into(),
            destination: PacketDestination::Broadcast,
          });
        }

        let mut message: String = match &nodes[&mesh_packet.from].user {
          Some(usr) => usr.long_name.clone(),
          None => mesh_packet.from.to_string(),
        };
        message.push_str(":\n");
        message.push_str(&decoded_text_message);

        return Some(Action::SendToGroup {
          message,
          master_key: config.group_key,
        });
      }
      _ => println!("invalid portnum but also shouldnt see this"),
    },

    meshtastic::protobufs::PortNum::WaypointApp => {
      let decoded_waypoint =
        meshtastic::protobufs::Waypoint::decode(packet_data.payload.as_slice()).unwrap();

      println!("Received waypoint packet: {:?}", decoded_waypoint);
    }
    _ => {
      println!(
        "Received mesh packet on port {:?}, not handling...",
        packet_data.portnum
      );
    }
  }

  None
}

pub type MyManager = Manager<SqliteStore, presage::manager::Registered>;

#[derive(Debug)]
pub struct Model {
  running_state: RunningState,
  contacts: Contacts,
  groups: Groups,
  // groups: Vec<Group,
  // chat_index: usize,
  account: Account,
}

impl Model {
  fn init(manager: &mut MyManager) -> Self {
    Model {
      account: Account {
        name: "nan".to_string(),
        username: "nan".to_string(),
        number: PhoneNumber("idc".to_string()),
        uuid: manager.registration_data().service_ids.aci,
      },
      groups: Default::default(),
      contacts: Default::default(),
      running_state: Default::default(),
    }
  }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum RunningState {
  #[default]
  Running,
  OhShit,
}

// #[derive(Debug, Default)]
// pub struct TimeStamps {
//   sent: DateTime<Utc>,
//   recieved: Option<DateTime<Utc>>,
//   readby: Option<Vec<(Contact, DateTime<Utc>)>>,
// }

#[derive(Debug)]
pub enum ReceiptType {
  Delivered,
  Read,
}

#[derive(Debug, Clone)]
pub struct Reaction {
  emoji: char,
  author: Uuid,
}

// pub struct MyImageWrapper(StatefulProtocol);

// sshhhhhh
// impl Debug for MyImageWrapper {
//   fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     Ok(())
//   }
// }

#[derive(Hash, PartialEq, Eq, Debug)]
struct PhoneNumber(String);

impl Clone for PhoneNumber {
  fn clone(&self) -> Self {
    PhoneNumber(self.0.clone())
  }
}

#[derive(Debug, Default)]
struct MyGroup {
  name: String,
  // icon: Option<MyImageWrapper>,
  _description: String,
  num_members: usize,
}

// #[derive(Debug, Default)]
// pub struct Contact {
//   _name: String,
//   nick_name: String,
//   // pfp: Option<MyImageWrapper>,
//   // icon: Image,
// }

type Contacts = Arc<HashMap<Uuid, Profile>>;
type Groups = Arc<HashMap<GroupMasterKeyBytes, Group>>;

#[derive(Debug)]
struct MessageOptions {
  opened: bool,
  index: usize,
  timestamp: u64,
  mine: bool,
  // my_actions: Vec<Action>,
  // not_my_actions: Vec<Action>,
}

#[derive(Debug)]
pub struct Chat {
  thread: Thread,
  display: MyGroup,
  // a little convenience field so u dont have to get that hash map every time
  // thread: Thread
}

pub struct Settings {
  _identity: String,
}

#[derive(Debug)]
struct Account {
  name: String,
  username: String,
  number: PhoneNumber,
  uuid: Uuid,
}

fn draw_linking_screen(url: &Option<Url>) {
  let _block = "██";

  let _size: u16 = 1;

  match &url {
    Some(url) => {
      let qr = QrCode::encode_text(&url.to_string(), QrCodeEcc::Medium);

      match qr {
        Ok(qr) => {
          // size = qr.size() as u16;
          print!("\x1b[37m");
          println!("{}", "██".repeat(qr.size() as usize + 2));
          for y in 0..qr.size() {
            print!("\x1b[37m");
            print!("██");
            for x in 0..qr.size() {
              match qr.get_module(x, y) {
                true => print!("\x1b[30m"),
                false => print!("\x1b[37m"),
              }
              print!("██");
              // (... paint qr.get_module(x, y) ...)
            }

            print!("\x1b[37m");
            print!("██");
            println!();
          }

          print!("\x1b[37m");
          println!("{}", "██".repeat(qr.size() as usize + 2));
        }

        Err(_) => println!("Error generating qrcode (tough shit pal)"),
      }
      // let raw_url = vec![Line::from("Or visit the raw url:"), Line::from(url.to_string())];
      println!("Or visit the url like a caveman: {}", url.to_string());
      // Paragraph::new(raw_url).render(
      //   Rect {
      //     x: area.x,
      //     y: area.y + size,
      //     width: area.width,
      //     height: area.height - size,
      //   },
      //   buffer,
      // );
    }

    None => println!("Generating Linking Url ..."),
  }
}

#[derive(Deserialize, Serialize)]
struct RawConfig {
  group_key: String,
}

#[derive(Deserialize, Serialize)]
struct Config {
  group_key: GroupMasterKeyBytes,
}

impl From<RawConfig> for Config {
  fn from(value: RawConfig) -> Self {
    let almost_key =
      hex::decode(value.group_key).expect("failed to parse key\nshould parese to a [u8; 32]");
    if almost_key.len() != 32 {
      panic!("incorrect key length: {}", almost_key.len());
    }
    let mut key: [u8; 32] = [0; 32];
    for (index, byte) in almost_key.iter().enumerate() {
      key[index] = *byte;
    }
    // let key: GroupMasterKeyBytes = key;

    Config { group_key: key }
  }
}

fn parse_config() -> Config {
  let mut file = match File::open("config.toml") {
    Ok(f) => f,
    Err(err) => {
      eprintln!("unable to open file 'config.toml'");
      eprintln!("heres an error also {}", err);
      panic!();
    }
  };
  let mut contents = String::new();
  file
    .read_to_string(&mut contents)
    .expect("cmon no way this fails");

  let raw: RawConfig = toml::from_str(&contents).expect("failed to parse config file");
  raw.into()
}

#[allow(unexpected_cfgs)]
#[tokio::main(flavor = "local")]
async fn main() -> anyhow::Result<()> {
  let (action_tx, mut action_rx) = mpsc::unbounded_channel();
  let db_path = default_db_path();
  let mut config_store =
    SqliteStore::open_with_passphrase(&db_path, "secret".into(), OnNewIdentity::Trust).await?;

  if !config_store.is_registered().await {
    link_device(
      SignalServers::Production,
      "meshtastic-2-signal".to_string(),
      action_tx.clone(),
    );

    // spawner.spawn(Cmd::LinkDevice {
    //   servers: SignalServers::Production,
    //   device_name: "terminal enjoyer".to_string(),
    // });
    //
    let mut url = None;

    loop {
      draw_linking_screen(&url);

      // Handle events and map to a Message
      let current_msg = action_rx.recv().await;

      match current_msg {
        Some(Action::Link(linking)) => match linking {
          LinkingAction::Url(new_url) => url = Some(new_url),
          LinkingAction::Success => break,
          LinkingAction::Fail => link_device(
            SignalServers::Production,
            "meshtastic-2-signal".to_string(),
            action_tx.clone(),
          ),
          //   spawner.spawn(Cmd::LinkDevice {
          //   servers: SignalServers::Production,
          //   device_name: "terminal enjoyer".to_string(),
          // }),
        },

        Some(Action::Quit) => {
          return Ok(());
        }

        Some(_) => {}

        None => {
          Logger::log("I dont think this should ever happenn".to_string());
        }
      }
    }

    // there probably a better way to make the store linked but this only happens once so idc
    config_store =
      SqliteStore::open_with_passphrase(&db_path, "secret".into(), OnNewIdentity::Trust).await?;
  }

  Logger::log("Linked!!!");

  let mut manager = Manager::load_registered(config_store)
    .await
    .expect("failed to make the manager");

  let groups = list_groups(&manager).await;
  println!("heres the groups for ur convenience: ");
  for group in groups {
    println!("key: {}, title; {}", hex::encode(group.0), group.1.title);
  }

  let config = parse_config();

  let mut model = Model::init(&mut manager);

  let spawner = SignalSpawner::new(manager, action_tx.clone());

  let stream_api = StreamApi::new();

  let available_ports = utils::stream::available_serial_ports()?;
  println!("Available ports: {:?}", available_ports);
  // println!("Enter the name of a port to connect to:");
  //
  let port = String::from("/dev/ttyACM0");

  let serial_stream = utils::stream::build_serial_stream(port, None, None, None)?;
  let (mut decoded_listener, stream_api) = stream_api.connect(serial_stream).await;

  let config_id = utils::generate_rand_id();
  let mut stream_api = stream_api.configure(config_id).await?;

  // let mut nodes = HashMap::<u32, meshtastic::protobufs::NodeInfo>::new();
  let mut nodes = Nodes::new();

  // let channel_config = protobufs::Channel {
  //   index: 1,
  //   settings: Some(ChannelSettings {
  //     psk: vec![],
  //     name: "gateway".to_string(),
  //     id: 67,
  //     uplink_enabled: true,
  //     downlink_enabled: true,
  //     module_settings: None,
  //     channel_num: 0,
  //   }),
  //   role: 2,
  // };

  let mut packet_router = DumbPacketRouter;
  // println!(
  //   "{:?}",
  //   stream_api
  //     .update_channel_config::<String, MyError, DumbPacketRouter>(&mut packet_router, channel_config)
  //     .await
  // );

  // This loop can be broken with ctrl+c or by disconnecting
  // the attached serial port.
  Logger::log("listening for mesh packets...");
  loop {
    // let soon_to_be_legacy = decoded_listener.recv().await;

    // let mut current_action = if let Some(decdoed) = soon_to_be_legacy {
    //   Some(Action::FromRadio(decdoed))
    // } else {
    //   break;
    // };

    let mut current_action = tokio::select! {
      decoded = decoded_listener.recv() => {
        if let Some(decdoed) = decoded {
          Some(Action::FromRadio(decdoed))
        } else {
        break;
      }}

      action = action_rx.recv() => {
        action
      }
    };

    while let Some(action) = current_action {
      current_action = match action {
        Action::FromRadio(decoded) => handle_from_radio_packet(decoded, &config, &mut nodes),

        Action::SendToMesh {
          body,
          channel,
          destination,
        } => {
          println!("\tsending to mesh...");
          println!(
            "{:?}",
            stream_api
              .send_mesh_packet(
                &mut packet_router,
                body.into_bytes().into(),
                protobufs::PortNum::TextMessageApp,
                destination,
                channel,
                true,
                false,
                true,
                None,
                None,
              )
              .await
          );
          None
        }
        Action::SendToGroup { message, master_key } => {
          spawner.spawn(Cmd::SendToGroup {
            message,
            master_key,
            timestamp: Utc::now().timestamp_millis() as u64,
            attachment_filepath: vec![],
          });
          None
        }
        Action::Receive(received) => match received {
          Received::Content(content) => handle_message(&mut model, &config, *content),
          Received::Contacts => {
            None
            // update our in memory cache of contacts
            // _ = update_contacts(model, spawner).await;
          }
          Received::QueueEmpty => None,
        },
        _ => None,
      }
    }
  }

  Ok(())
}
