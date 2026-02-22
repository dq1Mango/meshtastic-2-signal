mod simulators;

use meshtastic_2_signal::dumb_packet_router::DumbPacketRouter;
use meshtastic_2_signal::*;
use meshtastic::packet::PacketRouter;
use meshtastic::types::NodeId;
use simulators::*;

fn make_router() -> (
    DumbPacketRouter,
    tokio::sync::mpsc::UnboundedReceiver<Action>,
    tokio::sync::mpsc::UnboundedReceiver<u32>,
) {
    let (ack_tx, ack_rx) = tokio::sync::mpsc::unbounded_channel();
    let (id_tx, id_rx) = tokio::sync::mpsc::unbounded_channel();
    let router = DumbPacketRouter::new(NodeId::new(GATEWAY_NODE), ack_tx, id_tx);
    (router, ack_rx, id_rx)
}

#[test]
fn routing_ack_produces_mesh_ack_action() {
    let mut h = setup();
    let (mut router, mut ack_rx, mut id_rx) = make_router();

    // First, simulate sending a mesh packet through the router to register it
    let outgoing = MeshPacket {
        id: 42,
        from: GATEWAY_NODE,
        to: ALICE_NODE,
        want_ack: true,
        ..Default::default()
    };
    let _ = router.handle_mesh_packet(outgoing);

    // Should get the packet ID back
    let id = id_rx.try_recv().expect("should receive packet id");
    assert_eq!(id, 42);

    // Now feed a routing ack for that packet
    let ack = h.mesh.routing_ack(ALICE_NODE, 42);
    let _ = router.handle_packet_from_radio(ack);

    // Should produce a MeshAck action
    let action = ack_rx.try_recv().expect("should receive MeshAck");
    match action {
        Action::MeshAck { packet, deliverd } => {
            assert_eq!(packet.id, 42);
            assert!(deliverd);
        }
        other => panic!("expected MeshAck, got {:?}", other),
    }
}

#[test]
fn manual_round_trip_signal_to_mesh_ack() {
    let mut h = setup();
    let (mut router, mut ack_rx, mut _id_rx) = make_router();

    // Step 1: Signal message comes in, produces SendToMesh
    let content = h.signal.group_data_message(alice_uuid(), "test message");
    let action = meshtastic_2_signal::update::handle_message(&mut h.model, &h.config, content);
    let action = action.expect("should produce SendToMesh");

    match action {
        Action::SendToMesh {
            signal_message, ..
        } => {
            let signal_msg = signal_message.expect("should have signal_message");

            // Step 2: Simulate the packet being sent and getting an ID
            let packet_id: u32 = 500;
            h.model.mesh_to_signal.insert(packet_id, signal_msg.clone());

            // Step 3: Register the outgoing packet in the router
            let outgoing = MeshPacket {
                id: packet_id,
                want_ack: true,
                ..Default::default()
            };
            let _ = router.handle_mesh_packet(outgoing);

            // Step 4: Feed a routing ack
            let ack = h.mesh.routing_ack(ALICE_NODE, packet_id);
            let _ = router.handle_packet_from_radio(ack);

            // Step 5: Verify MeshAck
            let ack_action = ack_rx.try_recv().expect("should receive MeshAck");
            match ack_action {
                Action::MeshAck { packet, deliverd } => {
                    assert_eq!(packet.id, packet_id);
                    assert!(deliverd);

                    // Step 6: Simulate the main loop removing from mesh_to_signal
                    let removed = h.model.mesh_to_signal.remove(&packet.id);
                    assert!(removed.is_some());
                    assert_eq!(removed.unwrap().body, "test message");
                }
                other => panic!("expected MeshAck, got {:?}", other),
            }
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn ack_for_unknown_packet_id_produces_nothing() {
    let mut h = setup();
    let (mut router, mut ack_rx, _id_rx) = make_router();

    // Feed a routing ack for a packet ID that was never registered
    let ack = h.mesh.routing_ack(ALICE_NODE, 99999);
    let _ = router.handle_packet_from_radio(ack);

    // Should NOT produce a MeshAck (no packet in the pending map)
    assert!(ack_rx.try_recv().is_err(), "should not produce MeshAck for unknown ID");
}

#[test]
fn delivered_flag_is_true_on_ack() {
    let mut h = setup();
    let (mut router, mut ack_rx, _id_rx) = make_router();

    let outgoing = MeshPacket {
        id: 77,
        want_ack: true,
        ..Default::default()
    };
    let _ = router.handle_mesh_packet(outgoing);

    let ack = h.mesh.routing_ack(ALICE_NODE, 77);
    let _ = router.handle_packet_from_radio(ack);

    let action = ack_rx.try_recv().expect("should get MeshAck");
    match action {
        Action::MeshAck { deliverd, .. } => {
            assert!(deliverd, "deliverd flag should be true");
        }
        other => panic!("expected MeshAck, got {:?}", other),
    }
}
