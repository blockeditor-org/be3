use super::*;

/// A key as it travels, and the offer of one a share link hands on, print without the key, as
/// any `Debug` might end up in a log.
#[test]
fn a_debug_print_shows_no_key() {
    let carried = be_block_keys::CarriedKey {
        key: [0xab; 32],
        label: planted(Uuid::nil(), "scope", 1),
        block: [1; 16],
        root: None,
        heads: be_block_keys::Heads::default(),
    };
    let offer = crate::ScopeOffer {
        organization: Uuid::nil(),
        block: Uuid::nil(),
        access: be_block_graph::Access::View,
        key: Uuid::nil(),
        secret: carried.encode(),
    };
    let kept = crate::contacts::KeptLink {
        code: vec![0xab; 25],
        kind: be_block_protocol::LinkKind::Contact,
        made: 0,
    };
    let greeting = be_block_protocol::Greeting {
        token: Some("GREETINGTOKEN".into()),
    };
    let printed = format!("{carried:?} {offer:?} {kept:?} {greeting:?}");
    assert!(
        !printed.contains("171, 171") && !printed.contains("GREETINGTOKEN"),
        "a key, a code or a token shows in a debug print: {printed}"
    );
}
