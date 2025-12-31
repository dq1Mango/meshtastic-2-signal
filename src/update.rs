use presage::proto::data_message::{self, Quote};
use presage::proto::sync_message::Sent;

// use presage::model::messages::Received;
use presage::libsignal_service::content::{Content, ContentBody};
use presage::libsignal_service::prelude::ProfileKey;
use presage::proto::{DataMessage, SyncMessage};
use presage::store::ContentExt;
use presage::store::Thread;

use std::sync::Arc;

use crate::logger::Logger;
use crate::*;

#[derive(PartialEq, Debug)]
pub enum LinkingAction {
  Url(Url),
  Success,
  Fail,
}

#[derive(Debug, Copy, Clone)]
pub enum MessageOption {
  Reply,
  React,
  Edit,
  Copy,
  Info,
  Delete,
}

// the important one
#[derive(Debug)]
pub enum Action {
  SendToMesh {
    body: String,
    channel: MeshChannel,
    destination: PacketDestination,
  },

  FromRadio(FromRadio),

  SendToGroup {
    message: String,
    master_key: GroupMasterKeyBytes,
  },

  PickOption,
  DoOption(MessageOption),

  // Message(Content),
  Receive(Received),
  ReceiveBatch(Vec<Content>),

  Link(LinkingAction),
  Quit,
}

// pub async fn update(model: &mut Model, msg: Action, spawner: &SignalSpawner) -> Option<Action> {
//   match msg {
//     // Action::SetFocus(new_focus) => model.focus = new_focus,
//     Action::Receive(received) => match received {
//       Received::Content(content) => {
//         return handle_message(model, *content);
//       }
//       Received::Contacts => {
//         // update our in memory cache of contacts
//         _ = update_contacts(model, spawner).await;
//       }
//       Received::QueueEmpty => {}
//     },
//
//     Action::ReceiveBatch(received) => {
//       for message in received {
//         handle_message(model, message);
//       }
//     }
//
//     Action::Quit => {
//       // You can handle cleanup and exit here
//       // -- im ok thanks tho
//       model.running_state = RunningState::OhShit;
//     }
//
//     _ => {}
//   }
//
//   None
// }

// pub fn handle_option(
//   model: &mut Model,
//   spawner: &SignalSpawner,
//   option: MessageOption,
// ) -> Option<Action> {
//   let chat = model.current_chat();
//   let message = chat.find_message(chat.message_options.timestamp)?;
//
//   // ensure the optino we receive is actually valid for the message
//   // ie. cant edit / delete someone elses message
//   if let Metadata::NotMyMessage(_) = message.metadata {
//     match &option {
//       &MessageOption::Edit => {
//         Logger::log(format!("invalid message option: {:?}", option));
//         return None;
//       }
//       &MessageOption::Delete => {
//         Logger::log(format!("invalid message option: {:?}", option));
//         return None;
//       }
//       _ => {}
//     }
//   }
//
//   chat.message_options.close();
//
//   match option {
//     MessageOption::Copy => {
//       let result = execute!(
//         std::io::stdout(),
//         CopyToClipboard::to_clipboard_from(
//           &model.current_chat().selected_message().expect("kaboom").body.body
//         )
//       );
//
//       if let Err(error) = result {
//         Logger::log(error)
//       }
//
//       Some(Action::SetMode(Mode::Normal))
//     }
//     MessageOption::Reply => {
//       chat.text_input.mode = TextInputMode::Replying;
//       Some(Action::SetMode(Mode::Insert))
//     }
//     MessageOption::React => {
//       chat.text_input.mode = TextInputMode::Reacting;
//       Some(Action::SetMode(Mode::Insert))
//     }
//     MessageOption::Edit => {
//       // kinda gotta find the message twice sometimes cuz "cant have more than one mutable borrow
//       // yaaaaaaaaaayy..."
//       let body = chat
//         .find_message(chat.message_options.timestamp)?
//         .body
//         .body
//         .clone();
//       chat.text_input.set_content(body);
//
//       chat.text_input.mode = TextInputMode::Editing;
//       Some(Action::SetMode(Mode::Insert))
//     }
//     MessageOption::Delete => {
//       let ts = model.current_chat().message_options.timestamp;
//       spawner.spawn(Cmd::DeleteMessage {
//         thread: model.current_chat().thread.clone(),
//         target_timestamp: ts,
//       });
//
//       model.current_chat().delete_message(ts);
//
//       Some(Action::SetMode(Mode::Normal))
//     }
//     _ => None,
//   }
// }

pub fn handle_message(model: &mut Model, config: &Config, content: Content) -> Option<Action> {
  // Logger::log(format!("DataMessage: {:#?}", content.clone()));

  let ts = content.timestamp();
  let _timestamp = DateTime::from_timestamp_millis(ts as i64).expect("this happens too often");

  let Ok(mut thread) = Thread::try_from(&content) else {
    Logger::log("failed to derive thread from content".to_string());
    return None;
  };

  if let Thread::Group(group_key) = thread {
    if group_key != config.group_key {
      return None;
    }
  } else {
    return None;
  }

  match content.body {
    ContentBody::DataMessage(DataMessage {
      body: Some(body),
      quote,
      reaction,
      ..
    })
    | ContentBody::SynchronizeMessage(SyncMessage {
      sent:
        Some(Sent {
          message:
            Some(DataMessage {
              body: Some(body),
              quote,
              reaction,
              ..
            }),
          ..
        }),
      // read: read,
      ..
    }) => {
      // Logger::log(format!("DataMessage: {:#?}", body.clone()));
      // some flex-tape on the thread derivation
      let mut mine = false;
      if let Thread::Contact(uuid) = thread {
        if uuid == model.account.uuid {
          thread = Thread::Contact(content.metadata.destination.raw_uuid());
          mine = true;
        }
      }

      let _quote = if let Some(Quote { id, .. }) = quote {
        id
      } else {
        None
      };

      let _reactions = if let Some(data_message::Reaction {
        emoji: Some(emoji), ..
      }) = reaction
      {
        Logger::log("it works like this");
        vec![Reaction {
          emoji: emoji.chars().nth(0)?,
          author: content.metadata.sender.raw_uuid(),
        }]
      } else {
        vec![]
      };

      let mut message: String = format!("{:?}", content.metadata.sender);
      message.push_str(":\n");
      message.push_str(&body);

      Logger::log("broadcasting to mesh...");

      return Some(Action::SendToMesh {
        body: message,
        channel: 1.into(),
        destination: PacketDestination::Broadcast,
      });

      // insert_message(model, data, thread, ts, mine)
    }

    ContentBody::SynchronizeMessage(data) => {
      match data {
        SyncMessage {
          sent:
            Some(Sent {
              message:
                Some(DataMessage {
                  body: Some(_body),
                  quote: _,
                  ..
                }),
              ..
            }),
          // read: read,
          ..
        } => {
          // for receipt in read {
          //   let Some(aci) = receipt.sender_aci else {
          //     continue;
          //   };
          //   let Some(timestamp) = receipt.timestamp else { continue };
          //   let Some(aci) = ServiceId::parse_from_service_id_string(&aci) else {
          //     Logger::log("plz no".to_string());
          //     return None;
          //   };
          //   read_by.push(Receipt {
          //     sender: aci.raw_uuid(),
          //     timestamp: DateTime::from_timestamp_millis(timestamp as i64).expect("i think i gotta ditch chrono"),
          //   });
          // }
        }
        _ => {}
      }
    }

    ContentBody::DataMessage(DataMessage {
      body: None,
      reaction: Some(reaction),
      ..
    }) => {
      // some flex-tape on the thread derivation
      if let Thread::Contact(uuid) = thread {
        if uuid == model.account.uuid {
          thread = Thread::Contact(content.metadata.destination.raw_uuid());
        }
      }

      if let data_message::Reaction {
        emoji: Some(emoji),
        target_sent_timestamp: Some(_target_ts),
        ..
      } = reaction
      {
        let _reaction = Reaction {
          emoji: emoji.chars().nth(0)?,
          author: content.metadata.sender.raw_uuid(),
        };
      }

      // insert_message(model, data, thread, ts, mine)
    }
    _ => {}
  }

  None
}

pub async fn update_contacts(model: &mut Model, spawner: &SignalSpawner) -> anyhow::Result<()> {
  Logger::log("updating contacts".to_string());
  for contact in spawner.list_contacts().await? {
    // Logger::log(format!("{}", contact.inbox_position));
    if model.contacts.contains_key(&contact.uuid) {
      Logger::log("already_gyatt key".to_string());
      continue;
    } else {
      let profile_key = match contact.profile_key.clone().try_into() {
        Ok(bytes) => Some(ProfileKey::create(bytes)),
        Err(_) => {
          // Logger::log(format!("died on this dude: {:#?}", contact));
          None
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

impl Model {
  pub async fn update_groups(self: &mut Self, spawner: &SignalSpawner) -> anyhow::Result<()> {
    Logger::log("updating groups".to_string());
    for (key, group) in spawner.list_groups().await {
      if !self.groups.contains_key(&key) {}
      let Some(groups) = Arc::get_mut(&mut self.groups) else {
        Logger::log("didnt get off so easy".to_string());
        continue;
      };

      groups.insert(key, group);
    }
    Ok(())
  }
}
