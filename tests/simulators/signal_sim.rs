use std::collections::HashMap;
use std::sync::Arc;

use meshtastic_2_signal::{
    Contacts, GroupMasterKeyBytes, Profile, Uuid,
};
use presage::libsignal_service::content::{Content, ContentBody, Metadata};
use presage::libsignal_service::profile_name::ProfileName;
use presage::libsignal_service::protocol::{DeviceId, ServiceId};
use presage::proto::data_message::Quote;
use presage::proto::sync_message::Sent;
use presage::proto::{DataMessage, GroupContextV2, ReceiptMessage, SyncMessage};

pub struct SimSignalUser {
    pub uuid: Uuid,
    pub name: String,
}

pub struct SignalSimulator {
    users: HashMap<Uuid, SimSignalUser>,
    pub group_key: GroupMasterKeyBytes,
    next_timestamp: u64,
    pub our_uuid: Uuid,
}

impl SignalSimulator {
    pub fn new(our_uuid: Uuid, group_key: GroupMasterKeyBytes) -> Self {
        Self {
            users: HashMap::new(),
            group_key,
            next_timestamp: 1700000000000,
            our_uuid,
        }
    }

    pub fn add_user(&mut self, uuid: Uuid, name: &str) {
        self.users.insert(
            uuid,
            SimSignalUser {
                uuid,
                name: name.to_string(),
            },
        );
    }

    fn next_ts(&mut self) -> u64 {
        let ts = self.next_timestamp;
        self.next_timestamp += 1000;
        ts
    }

    /// Build the contacts map matching the Model's Contacts type
    pub fn build_contacts(&self) -> Contacts {
        let mut contacts = HashMap::new();
        for (uuid, user) in &self.users {
            contacts.insert(
                *uuid,
                Profile {
                    name: Some(ProfileName {
                        given_name: user.name.clone(),
                        family_name: None,
                    }),
                    ..Default::default()
                },
            );
        }
        Arc::new(contacts)
    }

    fn make_metadata(&self, sender: Uuid) -> Metadata {
        Metadata {
            sender: ServiceId::Aci(sender.into()),
            destination: ServiceId::Aci(self.our_uuid.into()),
            sender_device: DeviceId::new(1).expect("valid device id"),
            timestamp: 0, // will be overridden by DataMessage timestamp
            needs_receipt: false,
            unidentified_sender: false,
            was_plaintext: false,
            server_guid: None,
        }
    }

    fn group_context(&self, key: &GroupMasterKeyBytes) -> GroupContextV2 {
        GroupContextV2 {
            master_key: Some(key.to_vec()),
            revision: Some(0),
            ..Default::default()
        }
    }

    /// DataMessage in the configured bridge group
    pub fn group_data_message(&mut self, sender_uuid: Uuid, body: &str) -> Content {
        let ts = self.next_ts();
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::DataMessage(DataMessage {
                body: Some(body.to_string()),
                group_v2: Some(self.group_context(&self.group_key)),
                timestamp: Some(ts),
                ..Default::default()
            }),
        }
    }

    /// SyncMessage from our own account (sent from another device)
    pub fn group_sync_message(&mut self, body: &str) -> Content {
        let ts = self.next_ts();
        Content {
            metadata: self.make_metadata(self.our_uuid),
            body: ContentBody::SynchronizeMessage(SyncMessage {
                sent: Some(Sent {
                    message: Some(DataMessage {
                        body: Some(body.to_string()),
                        group_v2: Some(self.group_context(&self.group_key)),
                        timestamp: Some(ts),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        }
    }

    /// Message in a different group (should be ignored)
    pub fn wrong_group_message(&mut self, sender_uuid: Uuid, body: &str) -> Content {
        let ts = self.next_ts();
        let wrong_key: GroupMasterKeyBytes = [0x99; 32];
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::DataMessage(DataMessage {
                body: Some(body.to_string()),
                group_v2: Some(self.group_context(&wrong_key)),
                timestamp: Some(ts),
                ..Default::default()
            }),
        }
    }

    /// Direct message (no group_v2) — should be ignored by handle_message
    pub fn direct_message(&mut self, sender_uuid: Uuid, body: &str) -> Content {
        let ts = self.next_ts();
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::DataMessage(DataMessage {
                body: Some(body.to_string()),
                group_v2: None,
                timestamp: Some(ts),
                ..Default::default()
            }),
        }
    }

    /// /help command in bridge group
    pub fn help_command(&mut self, sender_uuid: Uuid) -> Content {
        self.group_data_message(sender_uuid, "/help")
    }

    /// /channel command in bridge group
    pub fn channel_command(&mut self, sender_uuid: Uuid) -> Content {
        self.group_data_message(sender_uuid, "/channel")
    }

    /// Reaction-only message (no body)
    pub fn reaction_message(&mut self, sender_uuid: Uuid, target_ts: u64) -> Content {
        let ts = self.next_ts();
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::DataMessage(DataMessage {
                body: None,
                group_v2: Some(self.group_context(&self.group_key)),
                timestamp: Some(ts),
                reaction: Some(presage::proto::data_message::Reaction {
                    emoji: Some("👍".to_string()),
                    remove: Some(false),
                    target_sent_timestamp: Some(target_ts),
                    target_author_aci: Some(sender_uuid.to_string()),
                }),
                ..Default::default()
            }),
        }
    }

    /// Delivery receipt (should be ignored)
    pub fn receipt_message(&mut self, sender_uuid: Uuid) -> Content {
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::ReceiptMessage(ReceiptMessage {
                r#type: Some(1), // DELIVERY
                timestamp: vec![self.next_ts()],
            }),
        }
    }

    /// Message with a quote
    pub fn message_with_quote(
        &mut self,
        sender_uuid: Uuid,
        body: &str,
        quoted_text: &str,
        quoted_ts: u64,
    ) -> Content {
        let ts = self.next_ts();
        Content {
            metadata: self.make_metadata(sender_uuid),
            body: ContentBody::DataMessage(DataMessage {
                body: Some(body.to_string()),
                group_v2: Some(self.group_context(&self.group_key)),
                timestamp: Some(ts),
                quote: Some(Quote {
                    id: Some(quoted_ts),
                    text: Some(quoted_text.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        }
    }

    /// Message from a UUID not in contacts
    pub fn unknown_user_message(&mut self, body: &str) -> Content {
        let unknown_uuid = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        self.group_data_message(unknown_uuid, body)
    }
}
