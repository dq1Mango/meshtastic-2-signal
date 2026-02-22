pub mod mesh_sim;
pub mod signal_sim;

use meshtastic_2_signal::{
    Config, GroupMasterKeyBytes, Model, Nodes, Uuid,
};

use mesh_sim::MeshSimulator;
use signal_sim::SignalSimulator;

pub const TEST_GROUP_KEY: GroupMasterKeyBytes = [0x42; 32];
pub const GATEWAY_NODE: u32 = 0x92345678;

pub const ALICE_NODE: u32 = 0xAAAA0001;
pub const BOB_NODE: u32 = 0xBBBB0002;

pub fn alice_uuid() -> Uuid {
    Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
}

pub fn bob_uuid() -> Uuid {
    Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
}

pub fn our_uuid() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
}

pub fn test_config() -> Config {
    Config {
        group_key: TEST_GROUP_KEY,
        channel_index: 0,
    }
}

pub struct TestHarness {
    pub model: Model,
    pub config: Config,
    pub nodes: Nodes,
    pub mesh: MeshSimulator,
    pub signal: SignalSimulator,
}

pub fn setup() -> TestHarness {
    let mut mesh = MeshSimulator::new(GATEWAY_NODE);
    mesh.add_node(ALICE_NODE, "Alice", "AL");
    mesh.add_node(BOB_NODE, "Bob", "BO");

    let mut signal = SignalSimulator::new(our_uuid(), TEST_GROUP_KEY);
    signal.add_user(alice_uuid(), "Alice");
    signal.add_user(bob_uuid(), "Bob");

    let contacts = signal.build_contacts();
    let nodes = mesh.build_nodes_map();
    let config = test_config();

    let model = Model::new_for_test(our_uuid(), contacts, Default::default());

    TestHarness {
        model,
        config,
        nodes,
        mesh,
        signal,
    }
}
