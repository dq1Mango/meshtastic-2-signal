use meshtastic::packet::PacketRouter;

pub struct DumbPacketRouter;

#[derive(Debug)]
pub enum MyError {
  Dumb,
}

impl std::fmt::Display for MyError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    Ok(())
  }
}

impl std::error::Error for MyError {}

impl PacketRouter<String, MyError> for DumbPacketRouter {
  fn handle_packet_from_radio(&mut self, packet: meshtastic::protobufs::FromRadio) -> Result<String, MyError> {
    println!("not rly handling packet from radio ngl");
    Ok("hi".to_string())
  }

  fn handle_mesh_packet(&mut self, packet: meshtastic::protobufs::MeshPacket) -> Result<String, MyError> {
    println!("not rly handling packet ngl");
    Ok("bruh".to_string())
  }

  fn source_node_id(&self) -> meshtastic::types::NodeId {
    meshtastic::types::NodeId::default()
  }
}
