pub mod dumb_packet_router;
pub mod logger;
pub mod meshy;
pub mod update;

use std::collections::HashMap;
use std::sync::Arc;

// Re-export presage types that modules access via `use crate::*`
pub use presage::libsignal_service::Profile;
pub use presage::libsignal_service::content::{Content, ContentBody};
pub use presage::libsignal_service::prelude::{ProfileKey, Uuid};
pub use presage::libsignal_service::protocol::{Aci, ServiceId};
pub use presage::libsignal_service::zkgroup::GroupMasterKeyBytes;
pub use presage::model::groups::Group;
pub use presage::model::messages::Received;
pub use presage::proto::DataMessage;
pub use presage::store::Thread;

// Re-export meshtastic types that modules access via `use crate::*`
pub use meshtastic::Message;
pub use meshtastic::packet::PacketDestination;
pub use meshtastic::protobufs;
pub use meshtastic::protobufs::{ChannelSettings, FromRadio, MeshPacket, NodeInfo};
pub use meshtastic::types::MeshChannel;

// Re-export other deps used by modules
pub use chrono::{DateTime, Utc};
pub use url::Url;

// Re-export key items from child modules for convenience
pub use meshy::{handle_from_radio_packet, handle_mesh_packet};
pub use update::{Action, LinkingAction, MessageOption};

// Type aliases
pub type Nodes = HashMap<u32, NodeInfo>;
pub type Contacts = Arc<HashMap<Uuid, Profile>>;
pub type Groups = Arc<HashMap<GroupMasterKeyBytes, Group>>;

// Shared types - fields are pub for test access

#[derive(Debug)]
pub struct Model {
    pub running_state: RunningState,
    pub contacts: Contacts,
    pub groups: Groups,
    pub channels: Vec<ChannelSettings>,
    pub mesh_to_signal: HashMap<u32, SignalMessage>,
    pub account: Account,
}

impl Model {
    pub fn new(uuid: Uuid) -> Self {
        Model {
            account: Account { uuid },
            groups: Default::default(),
            contacts: Default::default(),
            running_state: Default::default(),
            mesh_to_signal: HashMap::new(),
            channels: Vec::with_capacity(8),
        }
    }

    /// Test constructor with pre-populated contacts and groups
    pub fn new_for_test(uuid: Uuid, contacts: Contacts, groups: Groups) -> Self {
        Model {
            account: Account { uuid },
            groups,
            contacts,
            running_state: Default::default(),
            mesh_to_signal: HashMap::new(),
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

#[derive(Debug)]
pub struct Account {
    pub uuid: Uuid,
}

#[derive(Debug, Clone)]
pub struct SignalMessage {
    pub body: String,
    pub sender: Uuid,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: char,
    pub author: Uuid,
}

#[derive(Debug)]
pub enum ReceiptType {
    Delivered,
    Read,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub group_key: GroupMasterKeyBytes,
    pub channel_index: usize,
}
