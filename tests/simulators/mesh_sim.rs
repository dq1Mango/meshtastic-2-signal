use std::collections::HashMap;

use meshtastic_2_signal::protobufs::{self, FromRadio, MeshPacket, NodeInfo, User};
use meshtastic_2_signal::protobufs::from_radio::PayloadVariant;
use meshtastic_2_signal::protobufs::mesh_packet;
use meshtastic_2_signal::{ChannelSettings, Nodes};

pub struct SimNode {
    pub num: u32,
    pub long_name: String,
    pub short_name: String,
}

pub struct MeshSimulator {
    nodes: HashMap<u32, SimNode>,
    next_packet_id: u32,
    pub local_node_num: u32,
}

impl MeshSimulator {
    pub fn new(local_node_num: u32) -> Self {
        Self {
            nodes: HashMap::new(),
            next_packet_id: 1000,
            local_node_num,
        }
    }

    pub fn add_node(&mut self, num: u32, long_name: &str, short_name: &str) {
        self.nodes.insert(
            num,
            SimNode {
                num,
                long_name: long_name.to_string(),
                short_name: short_name.to_string(),
            },
        );
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_packet_id;
        self.next_packet_id += 1;
        id
    }

    /// Build the Nodes map that handle_from_radio_packet expects
    pub fn build_nodes_map(&self) -> Nodes {
        let mut nodes = Nodes::new();
        for (num, sim) in &self.nodes {
            nodes.insert(
                *num,
                NodeInfo {
                    num: *num,
                    user: Some(User {
                        id: format!("!{:08x}", num),
                        long_name: sim.long_name.clone(),
                        short_name: sim.short_name.clone(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            );
        }
        nodes
    }

    /// Build a text message FromRadio packet
    pub fn text_message(&mut self, from: u32, channel: u32, text: &str) -> FromRadio {
        let id = self.next_id();
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel,
                id,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(
                    protobufs::Data {
                        portnum: protobufs::PortNum::TextMessageApp.into(),
                        payload: text.as_bytes().to_vec(),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// DM (channel 0) shortcut
    pub fn dm(&mut self, from: u32, text: &str) -> FromRadio {
        self.text_message(from, 0, text)
    }

    /// Channel message (channel 1) shortcut
    pub fn channel_message(&mut self, from: u32, text: &str) -> FromRadio {
        self.text_message(from, 1, text)
    }

    /// /ping DM
    pub fn ping_dm(&mut self, from: u32) -> FromRadio {
        self.dm(from, "/ping")
    }

    /// /ping on channel 1
    pub fn ping_channel(&mut self, from: u32) -> FromRadio {
        self.channel_message(from, "/ping")
    }

    /// Invalid UTF-8 payload
    pub fn invalid_utf8_message(&mut self, from: u32, channel: u32) -> FromRadio {
        let id = self.next_id();
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel,
                id,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(
                    protobufs::Data {
                        portnum: protobufs::PortNum::TextMessageApp.into(),
                        payload: vec![0xFF, 0xFE, 0x80, 0x81],
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// Encrypted packet (should be ignored)
    pub fn encrypted_packet(&mut self, from: u32) -> FromRadio {
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel: 1,
                id: self.next_id(),
                payload_variant: Some(mesh_packet::PayloadVariant::Encrypted(vec![
                    0xDE, 0xAD, 0xBE, 0xEF,
                ])),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// MeshPacket with no payload variant
    pub fn no_payload_mesh_packet(&mut self, from: u32) -> FromRadio {
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel: 1,
                id: self.next_id(),
                payload_variant: None,
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// Position packet (should be ignored)
    pub fn position_packet(&mut self, from: u32) -> FromRadio {
        let id = self.next_id();
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel: 0,
                id,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(
                    protobufs::Data {
                        portnum: protobufs::PortNum::PositionApp.into(),
                        payload: vec![],
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// Routing ACK packet for DumbPacketRouter testing
    pub fn routing_ack(&mut self, from: u32, request_id: u32) -> FromRadio {
        let id = self.next_id();
        FromRadio {
            payload_variant: Some(PayloadVariant::Packet(MeshPacket {
                from,
                to: self.local_node_num,
                channel: 0,
                id,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(
                    protobufs::Data {
                        portnum: protobufs::PortNum::RoutingApp.into(),
                        payload: vec![],
                        request_id,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// NodeInfo config packet
    pub fn node_info_packet(&mut self, node_num: u32) -> FromRadio {
        let sim = self.nodes.get(&node_num);
        let (long_name, short_name) = match sim {
            Some(s) => (s.long_name.clone(), s.short_name.clone()),
            None => (format!("Node {:x}", node_num), format!("{:02x}", node_num & 0xFF)),
        };
        FromRadio {
            payload_variant: Some(PayloadVariant::NodeInfo(NodeInfo {
                num: node_num,
                user: Some(User {
                    id: format!("!{:08x}", node_num),
                    long_name,
                    short_name,
                    ..Default::default()
                }),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// Channel config packet
    pub fn channel_packet(&self, index: u32, name: &str, psk: Vec<u8>) -> FromRadio {
        FromRadio {
            payload_variant: Some(PayloadVariant::Channel(protobufs::Channel {
                index: index as i32,
                settings: Some(ChannelSettings {
                    name: name.to_string(),
                    psk,
                    ..Default::default()
                }),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// FromRadio with no payload variant at all
    pub fn empty_packet(&self) -> FromRadio {
        FromRadio {
            payload_variant: None,
            ..Default::default()
        }
    }
}
