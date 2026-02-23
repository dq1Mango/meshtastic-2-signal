mod simulators;

use meshtastic_2_signal::update::handle_message;
use meshtastic_2_signal::*;
use simulators::*;

#[test]
fn empty_string_mesh_message() {
    let mut h = setup();
    let packet = h.mesh.channel_message(ALICE_NODE, "");
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    // Empty string is valid UTF-8, should still bridge
    let action = action.expect("empty string should still produce action");
    match action {
        Action::SendToGroup { message, .. } => {
            assert!(message.contains("Alice"));
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn very_long_mesh_message() {
    let mut h = setup();
    let long_text: String = "A".repeat(1000);
    let packet = h.mesh.channel_message(ALICE_NODE, &long_text);
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("long message should produce action");
    match action {
        Action::SendToGroup { message, .. } => {
            assert!(message.contains(&long_text));
            assert!(message.len() > 1000);
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn unicode_emoji_mesh_message() {
    let mut h = setup();
    let packet = h.mesh.channel_message(ALICE_NODE, "Hello 🌍🎉🚀");
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("emoji message should produce action");
    match action {
        Action::SendToGroup { message, .. } => {
            assert!(message.contains("🌍🎉🚀"));
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn multi_line_mesh_message() {
    let mut h = setup();
    let packet = h.mesh.channel_message(ALICE_NODE, "line one\nline two\nline three");
    let action = handle_from_radio_packet(&mut h.model, &h.config, &mut h.nodes, packet);

    let action = action.expect("multi-line message should produce action");
    match action {
        Action::SendToGroup { message, .. } => {
            assert!(message.contains("line one\nline two\nline three"));
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn signal_empty_body_string() {
    let mut h = setup();
    let content = h.signal.group_data_message(alice_uuid(), "");
    let action = handle_message(&mut h.model, &h.config, content);

    // body: Some("") still matches the DataMessage pattern
    let action = action.expect("empty body should produce action");
    match action {
        Action::SendToMesh { body, .. } => {
            assert!(body.contains("Alice"));
            // Body format is "Alice:\n" with empty message after
            assert!(body.ends_with(":\n"));
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn multiple_channels_configured_and_channel_command() {
    let mut h = setup();

    // Configure channel_index 1 (second channel)
    h.config.channel_index = 1;

    // Push two channels into model
    h.model.channels.push(ChannelSettings {
        name: "primary".to_string(),
        psk: vec![0x11],
        ..Default::default()
    });
    h.model.channels.push(ChannelSettings {
        name: "bridge".to_string(),
        psk: vec![0x22, 0x33],
        ..Default::default()
    });

    let content = h.signal.channel_command(alice_uuid());
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("/channel should produce action");
    match action {
        Action::SendToGroup { message, .. } => {
            // Should show channel at index 1 ("bridge"), not index 0 ("primary")
            assert!(message.contains("bridge"), "should show the configured channel index");
            assert!(!message.contains("primary"));
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}
