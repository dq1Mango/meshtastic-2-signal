mod simulators;

use meshtastic_2_signal::*;
use simulators::*;

#[test]
fn channel_message_bridges_to_signal_group() {
    let mut h = setup();
    let packet = h.mesh.channel_message(ALICE_NODE, "Hello from mesh!");
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToGroup {
            message,
            master_key,
            ranges,
        } => {
            assert_eq!(master_key, TEST_GROUP_KEY);
            assert!(message.starts_with("Alice:\n"));
            assert!(message.contains("Hello from mesh!"));
            // First body range should bold the sender name "Alice"
            assert_eq!(ranges.len(), 1);
            assert_eq!(ranges[0].start, Some(0));
            assert_eq!(ranges[0].length, Some(5)); // "Alice" = 5 chars
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn dm_ping_responds_with_pong_to_sender() {
    let mut h = setup();
    let packet = h.mesh.ping_dm(ALICE_NODE);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToMesh {
            body,
            channel,
            destination,
            signal_message,
        } => {
            assert_eq!(body, "pong!");
            assert_eq!(channel, MeshChannel::from(0));
            assert!(matches!(destination, PacketDestination::Node(_)));
            assert!(signal_message.is_none());
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn channel_ping_responds_with_broadcast_pong() {
    let mut h = setup();
    let packet = h.mesh.ping_channel(ALICE_NODE);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToMesh {
            body,
            channel,
            destination,
            signal_message,
        } => {
            assert_eq!(body, "pong!");
            assert_eq!(channel, MeshChannel::from(1));
            assert!(matches!(destination, PacketDestination::Broadcast));
            assert!(signal_message.is_none());
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn invalid_utf8_on_channel_0_returns_none() {
    let mut h = setup();
    let packet = h.mesh.invalid_utf8_message(ALICE_NODE, 0);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "invalid UTF-8 DM should be ignored");
}

#[test]
fn invalid_utf8_on_channel_1_returns_none() {
    let mut h = setup();
    let packet = h.mesh.invalid_utf8_message(ALICE_NODE, 1);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "invalid UTF-8 channel message should be ignored");
}

#[test]
fn encrypted_packet_returns_none() {
    let mut h = setup();
    let packet = h.mesh.encrypted_packet(ALICE_NODE);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "encrypted packets should be ignored");
}

#[test]
fn no_payload_mesh_packet_returns_none() {
    let mut h = setup();
    let packet = h.mesh.no_payload_mesh_packet(ALICE_NODE);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "mesh packet with no payload should be ignored");
}

#[test]
fn position_packet_returns_none() {
    let mut h = setup();
    let packet = h.mesh.position_packet(ALICE_NODE);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "position packets should be ignored");
}

#[test]
fn empty_from_radio_returns_none() {
    let mut h = setup();
    let packet = h.mesh.empty_packet();
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "empty FromRadio should be ignored");
}

#[test]
fn node_info_populates_nodes_map() {
    let mut h = setup();
    let new_node: u32 = 0xCCCC0003;
    h.mesh.add_node(new_node, "Charlie", "CH");
    let packet = h.mesh.node_info_packet(new_node);

    // Clear nodes to verify it gets populated
    h.nodes.clear();
    assert!(!h.nodes.contains_key(&new_node));

    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "NodeInfo should not produce an action");
    assert!(h.nodes.contains_key(&new_node));
    assert_eq!(
        h.nodes[&new_node].user.as_ref().unwrap().long_name,
        "Charlie"
    );
}

#[test]
fn channel_config_populates_model() {
    let mut h = setup();
    assert!(h.model.channels.is_empty());

    let packet = h
        .mesh
        .channel_packet(0, "gateway", vec![0xAA, 0xBB, 0xCC]);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    assert!(action.is_none(), "channel config should not produce an action");
    assert_eq!(h.model.channels.len(), 1);
    assert_eq!(h.model.channels[0].name, "gateway");
    assert_eq!(h.model.channels[0].psk, vec![0xAA, 0xBB, 0xCC]);
}

#[test]
fn multiple_nodes_send_distinct_messages() {
    let mut h = setup();

    let packet1 = h.mesh.channel_message(ALICE_NODE, "Hi from Alice");
    let action1 = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet1);
    let action1 = action1.expect("Alice message should produce action");

    let packet2 = h.mesh.channel_message(BOB_NODE, "Hi from Bob");
    let action2 = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet2);
    let action2 = action2.expect("Bob message should produce action");

    match (action1, action2) {
        (
            Action::SendToGroup {
                message: msg1, ..
            },
            Action::SendToGroup {
                message: msg2, ..
            },
        ) => {
            assert!(msg1.contains("Alice"));
            assert!(msg1.contains("Hi from Alice"));
            assert!(msg2.contains("Bob"));
            assert!(msg2.contains("Hi from Bob"));
        }
        _ => panic!("both should be SendToGroup"),
    }
}

#[test]
fn unknown_channel_number_returns_none() {
    let mut h = setup();
    // Channel 5 is not 0 or 1, so it should be unrecognized
    let packet = h.mesh.text_message(ALICE_NODE, 5, "hello");
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
    assert!(action.is_none(), "unknown channel number should produce no action");
}

#[test]
#[should_panic]
fn unknown_node_on_channel_1_panics() {
    // This documents the existing bug: nodes[&from] panics for unknown nodes
    let mut h = setup();
    let unknown_node: u32 = 0xDEAD0000;
    // Don't add to nodes map, so lookup will panic
    let packet = h.mesh.text_message(unknown_node, 1, "hello");
    let _ = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);
}
