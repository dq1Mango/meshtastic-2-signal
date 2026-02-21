use std::collections::HashMap;

use meshtastic::{
  packet::PacketRouter,
  protobufs::{
    self, MeshPacket, PortNum,
    from_radio::{self, PayloadVariant},
    mesh_packet,
  },
  types::NodeId,
};
use tokio::sync::mpsc;

use crate::update::Action;

// pub enum AckStatus {
//   Sent,
//   Acked,
//   Deliverd,
// }

pub struct DumbPacketRouter {
  id: NodeId,
  want_ack_packets: HashMap<u32, MeshPacket>,
  ack_notifs: tokio::sync::mpsc::UnboundedSender<Action>,
  heres_your_id: mpsc::UnboundedSender<u32>,
}

impl DumbPacketRouter {
  pub fn new(id: NodeId, ack_notifs: mpsc::UnboundedSender<Action>, id_sender: mpsc::UnboundedSender<u32>) -> Self {
    Self {
      id,
      want_ack_packets: HashMap::new(),
      ack_notifs,
      heres_your_id: id_sender,
    }
  }
}

#[derive(Debug)]
pub enum MyError {
  Dumb,
}

impl std::fmt::Display for MyError {
  fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    Ok(())
  }
}

impl std::error::Error for MyError {}

impl PacketRouter<String, MyError> for DumbPacketRouter {
  fn handle_packet_from_radio(&mut self, packet: meshtastic::protobufs::FromRadio) -> Result<String, MyError> {
    // oh man i sure hope no onne will have to understand this in the future
    // ... (that someone is me)
    if let Some(from_radio::PayloadVariant::Packet(MeshPacket {
      payload_variant: Some(mesh_packet::PayloadVariant::Decoded(ref decoded)),
      ..
    })) = packet.payload_variant
    {
      if decoded.portnum == Into::<i32>::into(PortNum::RoutingApp) {
        if decoded.request_id != 0 {
          if let Some(packet) = self.want_ack_packets.remove(&decoded.request_id) {
            self.ack_notifs.send(Action::MeshAck { packet, deliverd: true });
          }
        } else {
          println!("dont th9ink this should happen");
        }
      }
    }

    Ok("hi".to_string())
  }

  fn handle_mesh_packet(&mut self, packet: meshtastic::protobufs::MeshPacket) -> Result<String, MyError> {
    println!("not rly handling packet ngl");
    // println!("here it is anyway: {:?}", packet);

    self.heres_your_id.send(packet.id);

    if packet.want_ack {
      self.want_ack_packets.insert(packet.id, packet);
    }

    Ok("bruh".to_string())
  }

  fn source_node_id(&self) -> meshtastic::types::NodeId {
    self.id
    // NodeId::new(2454871382)
  }
}
