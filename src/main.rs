mod logger;
mod meshy;
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
use crate::meshy::*;
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
use meshtastic::protobufs::{Channel, ChannelSettings, FromRadio, NodeInfo, User, mesh_packet};
use meshtastic::types::MeshChannel;
use meshtastic::utils;

// This import allows for decoding of mesh packets
// Re-export of prost::Message
use meshtastic::Message;
use meshtastic::protobufs;

type Nodes = HashMap<u32, meshtastic::protobufs::NodeInfo>;

pub type MyManager = Manager<SqliteStore, presage::manager::Registered>;

type Contacts = Arc<HashMap<Uuid, Profile>>;
type Groups = Arc<HashMap<GroupMasterKeyBytes, Group>>;

#[derive(Debug)]
pub struct Model {
  running_state: RunningState,
  contacts: Contacts,
  groups: Groups,
  channels: Vec<ChannelSettings>,
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
      // 8 configurable channels
      channels: Vec::with_capacity(8),
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

#[derive(Deserialize, Serialize)]
struct RawConfig {
  group_key: String,
  channel_index: usize,
}

#[derive(Deserialize, Serialize)]
struct Config {
  group_key: GroupMasterKeyBytes,
  channel_index: usize,
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

    Config {
      group_key: key,
      channel_index: value.channel_index,
    }
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
        Action::FromRadio(decoded) => handle_from_radio_packet(&mut model, &config, &mut nodes, decoded),

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
        Action::SendToGroup {
          message,
          ranges,
          master_key,
        } => {
          spawner.spawn(Cmd::SendToGroup {
            message,
            ranges,
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
