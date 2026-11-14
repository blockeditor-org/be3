use super::*;

/// Unhurried, a type module seals a block after a quiet spell, as for a person: a timer, not
/// work, so a driver timing person-paced behaviour (`drive hurry off`) is not held back by it
/// for the length of that spell. Hurried, the host seals what is owed on every frame, so an owed
/// seal that stays is work that keeps the host busy.
#[test]
fn an_owed_seal_keeps_only_a_hurried_driver_waiting() {
    let block = Uuid::new_v4();
    let shared = Shared {
        owing: 1,
        owing_blocks: vec![be_block_type_host::Owing {
            block,
            content_type: Uuid::new_v4(),
            publish_error: None,
            journaled: true,
            unreported: false,
            publishing: 0,
            joining: false,
            closing: false,
        }],
        ..Shared::default()
    };
    assert!(
        shared
            .busy(true)
            .iter()
            .any(|reason| reason.contains("owes edits a seal") && reason.contains(&block.to_string())),
        "hurried, the host seals at once, so an owed seal is waited for: {:?}",
        shared.busy(true)
    );
    assert_eq!(
        shared.busy(false),
        Vec::<String>::new(),
        "unhurried, the seal waits for a quiet spell, which a driver does not wait for"
    );
}
