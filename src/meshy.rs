use presage::proto::{
  BodyRange,
  body_range::{AssociatedValue, Style},
};

use crate::*;

/// A helper function to handle packets coming directly from the radio connection.
/// The Meshtastic `PhoneAPI` will return decoded `FromRadio` packets, which
/// can then be handled based on their payload variant. Note that the payload
/// variant can be `None`, in which case the packet should be ignored.
pub fn handle_from_radio_packet(
  model: &mut Model,
  config: &Config,
  nodes: &mut Nodes,
  from_radio_packet: meshtastic::protobufs::FromRadio,
) -> Option<Action> {
  // let cloned_packet = from_radio_packet.clone();
  // Remove `None` variants to get the payload variant
  let payload_variant = match from_radio_packet.payload_variant {
    Some(payload_variant) => payload_variant,
    None => {
      println!("Received FromRadio packet with no payload variant, not handling...");
      return None;
    }
  };

  // `FromRadio` packets can be differentiated based on their payload variant,
  // which in Rust is represented as an enum. This means the payload variant
  // can be matched on, and the appropriate user-defined action can be taken.
  match payload_variant {
    meshtastic::protobufs::from_radio::PayloadVariant::Channel(channel) => {
      println!("Received channel packet: {:?}", channel);
      // just gyatta be sure
      // assert_eq!(model.channels.len(), channel.index as usize);

      if let Some(settings) = channel.settings {
        model.channels.push(settings);
      }
    }
    meshtastic::protobufs::from_radio::PayloadVariant::NodeInfo(node_info) => {
      nodes.insert(node_info.num, node_info);
    }
    meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet) => {
      return handle_mesh_packet(mesh_packet, nodes, config);
    }
    _ => {
      // println!("Received other FromRadio packet, not handling...");
    }
  };

  None
}

/// A helper function to handle `MeshPacket` messages, which are a subset
/// of all `FromRadio` messages. Note that the payload variant can be `None`,
/// and that the payload variant can be `Encrypted`, in which case the packet
/// should be ignored within client applications.
///
/// Mesh packets are the most commonly used type of packet, and are usually
/// what people are referring to when they talk about "packets."
pub fn handle_mesh_packet(
  mesh_packet: protobufs::MeshPacket,
  nodes: &Nodes,
  config: &Config,
) -> Option<Action> {
  // let cloned_packet = mesh_packet.clone();
  // println!("look at this fun packet: {:?}", cloned_packet);
  // Remove `None` variants to get the payload variant

  // Only handle decoded (unencrypted) mesh packets
  let packet_data = match mesh_packet.payload_variant {
    Some(protobufs::mesh_packet::PayloadVariant::Decoded(decoded_mesh_packet)) => decoded_mesh_packet,
    Some(protobufs::mesh_packet::PayloadVariant::Encrypted(_encrypted_mesh_packet)) => {
      // println!("Received encrypted mesh packet, not handling...");
      return None;
    }
    None => {
      println!("Received mesh packet with no payload variant, not handling...");
      return None;
    }
  };

  // Meshtastic differentiates mesh packets based on a field called `portnum`.
  // Meshtastic defines a set of standard port numbers [here](https://meshtastic.org/docs/development/firmware/portnum),
  // but also allows for custom port numbers to be used.
  match packet_data.portnum() {
    meshtastic::protobufs::PortNum::PositionApp => {
      // Note that `Data` structs contain a `payload` field, which is a vector of bytes.
      // This data needs to be decoded into a protobuf struct, which is shown below.
      // The `decode` function is provided by the `prost` crate, which is re-exported
      // by the `meshtastic` crate.
      let decoded_position =
        meshtastic::protobufs::Position::decode(packet_data.payload.as_slice()).unwrap();

      println!("Received position packet: {:?}", decoded_position);
    }

    meshtastic::protobufs::PortNum::TextMessageApp => match mesh_packet.channel {
      0 => {
        // println!("heres the whole packet: {:#?}", &cloned_packet);
        let decoded_text_message = String::from_utf8(packet_data.payload).unwrap();

        println!("Received DM message: {:?}", &decoded_text_message);

        if decoded_text_message == "/ping" {
          return Some(Action::SendToMesh {
            body: "pong!".to_string(),
            channel: 0.into(),
            destination: PacketDestination::Node(mesh_packet.from.into()),
          });
        }
      }
      1 => {
        // println!("heres the whole packet: {:#?}", &cloned_packet);
        let decoded_text_message = String::from_utf8(packet_data.payload).unwrap();

        println!("Received text message from channel: {:?}", &decoded_text_message);

        if decoded_text_message == "/ping" {
          return Some(Action::SendToMesh {
            body: "pong!".to_string(),
            channel: 1.into(),
            destination: PacketDestination::Broadcast,
          });
        }

        let name = match &nodes[&mesh_packet.from].user {
          Some(usr) => usr.long_name.clone(),
          None => format!("{:x}", mesh_packet.from),
        };

        let message = format!("{}:\n{}", name, decoded_text_message);

        return Some(Action::SendToGroup {
          message,
          master_key: config.group_key,
          ranges: vec![BodyRange {
            start: Some(0),
            length: Some(name.len() as u32),
            associated_value: Some(AssociatedValue::Style(Style::Bold.into())),
          }],
        });
      }
      _ => println!("invalid portnum but also shouldnt see this"),
    },

    meshtastic::protobufs::PortNum::WaypointApp => {
      let decoded_waypoint =
        meshtastic::protobufs::Waypoint::decode(packet_data.payload.as_slice()).unwrap();

      println!("Received waypoint packet: {:?}", decoded_waypoint);
    }
    _ => {
      println!(
        "Received mesh packet on port {:?}, not handling...",
        packet_data.portnum
      );
    }
  }

  None
}
