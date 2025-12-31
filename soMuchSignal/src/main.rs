mod logger;
mod mysignal;
mod signal;
mod update;

use std::{
  cmp,
  collections::HashMap,
  fmt::Debug,
  hash::Hash,
  io::{self, Stdout},
  sync::{Arc, Mutex},
  vec,
};

use presage::{
  libsignal_service::{
    Profile,
    configuration::SignalServers,
    prelude::{ProfileKey, Uuid},
    proto,
    zkgroup::GroupMasterKeyBytes,
  },
  model::groups::Group,
  store::Thread,
};

use presage::manager::Manager;
use presage::model::messages::Received;
use presage::store::{StateStore, Store};
use presage_store_sqlite::{OnNewIdentity, SqliteStore};
// use crate::database::{OnNewIdentity, SqliteStore};

use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::mpsc;
use url::Url;
// use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};

use qrcodegen::QrCode;
use qrcodegen::QrCodeEcc;
// use crate::signal::*;
use crate::signal::link_device;
use crate::update::*;
use crate::{logger::Logger, mysignal::SignalSpawner, signal::Cmd, update::LinkingAction};

pub type MyManager = Manager<SqliteStore, presage::manager::Registered>;
// #[derive(Debug, Default)]
pub struct Model {
  running_state: RunningState,
  contacts: Contacts,
  groups: Groups,
  // groups: Vec<Group,
  // chat_index: usize,
  account: Account,
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
  borders: bool,
  message_width_ratio: f32,
  _identity: String,
}

struct Account {
  name: String,
  username: String,
  number: PhoneNumber,
  uuid: Uuid,
}

fn draw_linking_screen(url: &Option<Url>) {
  let block = "██";

  let mut size: u16 = 1;

  match &url {
    Some(url) => {
      let qr = QrCode::encode_text(&url.to_string(), QrCodeEcc::Medium);

      match qr {
        Ok(qr) => {
          // size = qr.size() as u16;
          for y in 0..qr.size() {
            for x in 0..qr.size() {
              match qr.get_module(x, y) {
                true => print!("\x1b[30m"),
                false => print!("\x1b[37m"),
              }
              print!("██");
              // (... paint qr.get_module(x, y) ...)
            }
            println!();
          }
        }

        Err(_) => println!("Error generating qrcode (tough shit pal)"),
      }
      // let raw_url = vec![Line::from("Or visit the raw url:"), Line::from(url.to_string())];
      println!("Or visit the raw url: {}", url.to_string());
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
  let db_path = "/home/mqngo/Coding/soMuchSignal/signal.db3";
  let mut config_store =
    SqliteStore::open_with_passphrase(&db_path, "secret".into(), OnNewIdentity::Trust).await?;

  if !config_store.is_registered().await {
    link_device(
      SignalServers::Production,
      "terminal enjoyer".to_string(),
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

  let manager = Manager::load_registered(config_store)
    .await
    .expect("failed to make the manager");

  let spawner = SignalSpawner::new(manager, action_tx.clone());

  Ok(())
}
