use std::collections::HashMap;

use meshtastic::{
  packet::PacketRouter,
  protobufs::{self, MeshPacket, PortNum, mesh_packet::PayloadVariant},
  types::NodeId,
};
use tokio::sync::mpsc;

// pub enum AckStatus {
//   Sent,
//   Acked,
//   Deliverd,
// }

pub struct DumbPacketRouter {
  id: NodeId,
  want_ack_packets: HashMap<u32, MeshPacket>,
  ack_notifs: tokio::sync::mpsc::UnboundedSender<MeshPacket>,
}

impl DumbPacketRouter {
  pub fn new(id: NodeId, ack_notifs: mpsc::UnboundedSender<MeshPacket>) -> Self {
    Self {
      id,
      want_ack_packets: HashMap::new(),
      ack_notifs,
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
  fn handle_packet_from_radio(&mut self, _packet: meshtastic::protobufs::FromRadio) -> Result<String, MyError> {
    println!("not rly handling packet from radio ngl");
    Ok("hi".to_string())
  }

  fn handle_mesh_packet(&mut self, packet: meshtastic::protobufs::MeshPacket) -> Result<String, MyError> {
    println!("not rly handling packet ngl");
    println!("here it is anyway: {:?}", packet);

    if let Some(PayloadVariant::Decoded(ref decoded)) = packet.payload_variant {
      if decoded.portnum == Into::<i32>::into(PortNum::RoutingApp) {
        if decoded.request_id != 0 {
          if let Some(packet) = self.want_ack_packets.remove(&decoded.request_id) {
            self.ack_notifs.send(packet);
          }
        } else {
          println!("dont th9ink this should happen");
        }
      }
    }

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
