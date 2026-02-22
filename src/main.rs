mod mysignal;
mod signal;

use std::io::prelude::*;
use std::fs::File;
use std::sync::Arc;

use meshtastic_2_signal::*;

use presage::libsignal_service::configuration::SignalServers;
use presage::libsignal_service::prelude::ProfileKey;
use presage::manager::Manager;
use presage::store::StateStore;
use presage_store_sqlite::{OnNewIdentity, SqliteStore};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use qrcodegen::QrCode;
use qrcodegen::QrCodeEcc;
use crate::signal::{Cmd, config_dir_path, link_device};
use crate::signal::{default_db_path, list_groups};
use crate::mysignal::SignalSpawner;

use meshtastic::api::StreamApi;
use meshtastic::packet::PacketRouter;
use meshtastic::types::NodeId;
use meshtastic::utils;

use meshtastic_2_signal::dumb_packet_router::DumbPacketRouter;

pub type MyManager = Manager<SqliteStore, presage::manager::Registered>;

#[derive(Deserialize, Serialize)]
struct RawConfig {
    group_key: String,
    channel_index: usize,
}

impl From<RawConfig> for Config {
    fn from(value: RawConfig) -> Self {
        let almost_key = hex::decode(value.group_key).expect("failed to parse key\nshould parese to a [u8; 32]");
        if almost_key.len() != 32 {
            panic!("incorrect key length: {}", almost_key.len());
        }
        let mut key: [u8; 32] = [0; 32];
        for (index, byte) in almost_key.iter().enumerate() {
            key[index] = *byte;
        }

        Config {
            group_key: key,
            channel_index: value.channel_index,
        }
    }
}

fn config_path() -> String {
    let mut dir = config_dir_path();
    dir.push_str("config.toml");
    dir
}

fn parse_config() -> Config {
    let mut file = match File::open(config_path()) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("unable to open file 'config.toml'");
            eprintln!("heres an error also {}", err);
            panic!();
        }
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("cmon no way this fails");

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
            println!("Or visit the url like a caveman: {}", url.to_string());
        }

        None => println!("Generating Linking Url ..."),
    }
}

// Moved from update.rs - depends on SignalSpawner which lives in the binary crate
pub async fn update_contacts(model: &mut Model, spawner: &SignalSpawner) -> anyhow::Result<()> {
    use meshtastic_2_signal::logger::Logger;
    Logger::log("updating contacts".to_string());
    for contact in spawner.list_contacts().await? {
        if model.contacts.contains_key(&contact.uuid) {
            Logger::log("already_gyatt key".to_string());
            continue;
        } else {
            let profile_key = match contact.profile_key.clone().try_into() {
                Ok(bytes) => Some(ProfileKey::create(bytes)),
                Err(_) => {
                    Logger::log(format!("died on this dude: {:#?}", contact));
                    continue;
                }
            };

            let profile = match spawner.retrieve_profile(contact.uuid, profile_key).await {
                Ok(x) => x,
                Err(_) => continue,
            };

            let Some(contacts) = Arc::get_mut(&mut model.contacts) else {
                Logger::log("didnt get off so easy".to_string());
                return Ok(());
            };

            contacts.insert(contact.uuid, profile.clone());
        }
    }
    Ok(())
}

#[allow(unexpected_cfgs)]
#[tokio::main(flavor = "local")]
async fn main() -> anyhow::Result<()> {
    use meshtastic_2_signal::logger::Logger;

    let (action_tx, mut action_rx) = mpsc::unbounded_channel();
    let db_path = default_db_path();
    let mut config_store = SqliteStore::open_with_passphrase(&db_path, "secret".into(), OnNewIdentity::Trust).await?;

    if !config_store.is_registered().await {
        link_device(
            SignalServers::Production,
            "meshtastic-2-signal".to_string(),
            action_tx.clone(),
        );

        let mut url = None;

        loop {
            draw_linking_screen(&url);

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

        config_store = SqliteStore::open_with_passphrase(&db_path, "secret".into(), OnNewIdentity::Trust).await?;
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
    let thread = Thread::Group(config.group_key);

    let mut model = Model::new(manager.registration_data().service_ids.aci);

    let spawner = SignalSpawner::new(manager, action_tx.clone());
    let _result = update_contacts(&mut model, &spawner).await;

    let stream_api = StreamApi::new();

    let available_ports = utils::stream::available_serial_ports()?;
    println!("Available ports: {:?}", available_ports);

    let port = String::from("/dev/ttyACM0");

    let serial_stream = utils::stream::build_serial_stream(port, None, None, None)?;
    let (mut decoded_listener, stream_api) = stream_api.connect(serial_stream).await;

    let config_id = utils::generate_rand_id();
    let mut stream_api = stream_api.configure(config_id).await?;

    let mut nodes = Nodes::new();

    let (packet_id_tx, mut packet_id_rx) = mpsc::unbounded_channel::<u32>();
    let mut packet_router = DumbPacketRouter::new(NodeId::new(2454871382), action_tx.clone(), packet_id_tx);

    Logger::log("listening for mesh packets...");
    loop {
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
                Action::FromRadio(decoded) => {
                    packet_router.handle_packet_from_radio(decoded.clone());
                    handle_from_radio_packet(&mut model, &config, &mut nodes, decoded)
                }

                Action::SendToMesh {
                    body,
                    channel,
                    destination,
                    signal_message,
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

                    if let Some(message) = signal_message {
                        if let Some(id) = packet_id_rx.recv().await {
                            println!("\tthis is our id: {}", id);
                            model.mesh_to_signal.insert(id, message);
                        }
                    }
                    None
                }
                Action::SendToGroup {
                    message,
                    ranges,
                    master_key,
                } => {
                    println!("\tsending to signal...");
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
                    Received::Content(content) => meshtastic_2_signal::update::handle_message(&mut model, &config, *content),
                    Received::Contacts => {
                        _ = update_contacts(&mut model, &spawner).await;
                        None
                    }
                    Received::QueueEmpty => None,
                },

                Action::MeshAck { packet, deliverd } => {
                    println!("got ack!!!");
                    if deliverd {
                        if let Some(message) = model.mesh_to_signal.remove(&packet.id) {
                            spawner.spawn(Cmd::ReactToThread {
                                thread: thread.clone(),
                                reaction: "✔️".to_string(),
                                timestamp: Utc::now().timestamp_millis() as u64,
                                target_timestamp: message.timestamp,
                                author_uuid: Some(message.sender),
                            });
                        }
                    }

                    None
                }
                _ => None,
            }
        }
    }

    Ok(())
}
