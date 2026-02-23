mod simulators;

use meshtastic_2_signal::update::handle_message;
use meshtastic_2_signal::*;
use simulators::*;

#[test]
fn group_message_bridges_to_mesh() {
    let mut h = setup();
    let content = h.signal.group_data_message(alice_uuid(), "Hello from Signal!");
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToMesh {
            body,
            channel,
            destination,
            signal_message,
        } => {
            assert!(body.contains("Alice"));
            assert!(body.contains("Hello from Signal!"));
            assert_eq!(channel, MeshChannel::from(1));
            assert!(matches!(destination, PacketDestination::Broadcast));
            let sm = signal_message.expect("should have signal_message");
            assert_eq!(sm.sender, alice_uuid());
            assert_eq!(sm.body, "Hello from Signal!");
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn help_command_returns_help_text() {
    let mut h = setup();
    let content = h.signal.help_command(alice_uuid());
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToGroup {
            message,
            master_key,
            ..
        } => {
            assert_eq!(master_key, TEST_GROUP_KEY);
            assert!(message.contains("/channel"));
            assert!(message.contains("/help"));
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn channel_command_returns_channel_details() {
    let mut h = setup();
    // Populate a channel in the model first
    h.model.channels.push(ChannelSettings {
        name: "gateway".to_string(),
        psk: vec![0x01, 0x02, 0x03],
        ..Default::default()
    });

    let content = h.signal.channel_command(alice_uuid());
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToGroup {
            message,
            master_key,
            ranges,
        } => {
            assert_eq!(master_key, TEST_GROUP_KEY);
            assert!(message.contains("Channel Details:"));
            assert!(message.contains("gateway"));
            // Should have a bold range for "Channel Details:"
            assert!(!ranges.is_empty());
        }
        other => panic!("expected SendToGroup, got {:?}", other),
    }
}

#[test]
fn wrong_group_returns_none() {
    let mut h = setup();
    let content = h.signal.wrong_group_message(alice_uuid(), "wrong group");
    let action = handle_message(&mut h.model, &h.config, content);
    assert!(action.is_none(), "messages from wrong group should be ignored");
}

#[test]
fn direct_message_returns_none() {
    let mut h = setup();
    let content = h.signal.direct_message(alice_uuid(), "hey");
    let action = handle_message(&mut h.model, &h.config, content);
    assert!(action.is_none(), "DMs (no group) should be ignored");
}

#[test]
fn reaction_only_message_returns_none() {
    let mut h = setup();
    let content = h.signal.reaction_message(alice_uuid(), 12345);
    let action = handle_message(&mut h.model, &h.config, content);
    assert!(action.is_none(), "reaction-only messages should be ignored");
}

#[test]
fn receipt_message_returns_none() {
    let mut h = setup();
    let content = h.signal.receipt_message(alice_uuid());
    let action = handle_message(&mut h.model, &h.config, content);
    assert!(action.is_none(), "receipt messages should be ignored");
}

#[test]
fn sync_message_bridges_to_mesh() {
    let mut h = setup();
    // Add our own UUID to contacts so the name lookup works
    let our = our_uuid();
    {
        let contacts = std::sync::Arc::get_mut(&mut h.model.contacts).unwrap();
        contacts.insert(
            our,
            Profile {
                name: Some(presage::libsignal_service::profile_name::ProfileName {
                    given_name: "BridgeBot".to_string(),
                    family_name: None,
                }),
                ..Default::default()
            },
        );
    }

    let content = h.signal.group_sync_message("synced from another device");
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("sync message should produce an action");
    match action {
        Action::SendToMesh { body, .. } => {
            assert!(body.contains("synced from another device"));
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn unknown_contact_uses_uuid_fallback() {
    let mut h = setup();
    let content = h.signal.unknown_user_message("from nobody");
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("should produce an action");
    match action {
        Action::SendToMesh { body, .. } => {
            // Should contain the UUID representation as the name
            assert!(body.contains("from nobody"));
            // Should NOT contain "Alice" or "Bob" since this is an unknown user
            assert!(!body.contains("Alice"));
            assert!(!body.contains("Bob"));
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn message_with_quote_still_bridges() {
    let mut h = setup();
    let content = h
        .signal
        .message_with_quote(alice_uuid(), "reply text", "original", 99999);
    let action = handle_message(&mut h.model, &h.config, content);

    let action = action.expect("quoted message should produce an action");
    match action {
        Action::SendToMesh { body, .. } => {
            assert!(body.contains("reply text"));
            assert!(body.contains("Alice"));
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}

#[test]
fn empty_body_matches_but_bridges() {
    let mut h = setup();
    let content = h.signal.group_data_message(alice_uuid(), "");
    let action = handle_message(&mut h.model, &h.config, content);

    // Empty body "" still matches `body: Some(body)` pattern
    let action = action.expect("empty body should still produce an action");
    match action {
        Action::SendToMesh { body, .. } => {
            assert!(body.contains("Alice"));
        }
        other => panic!("expected SendToMesh, got {:?}", other),
    }
}
