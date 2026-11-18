use super::*;

/// A key as it travels, and a link's pending grant, print without the key, as any `Debug` might
/// end up in a log.
#[test]
fn a_debug_print_shows_no_key() {
    let carried = be_block_keys::CarriedKey {
        key: [0xab; 32],
        label: planted(Uuid::nil(), "scope", 1),
        block: [1; 16],
        root: None,
        heads: be_block_keys::Heads::default(),
    };
    let offer = crate::GrantOffer {
        organization: Uuid::nil(),
        block: Uuid::nil(),
        access: be_block_graph::Access::View,
        key: Uuid::nil(),
        secret: carried.encode(),
    };
    let printed = format!("{carried:?} {offer:?}");
    assert!(
        !printed.contains("171, 171"),
        "a key shows in a debug print: {printed}"
    );
}
