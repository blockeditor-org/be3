use super::*;

#[test]
fn a_block_gift_for_a_block_the_host_refused_to_make_never_starts() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let mut instances = placed();
    let refused = Uuid::now_v7();
    call(
        &mut instances,
        Call::CreateBlock {
            reply: reply(next_op()),
            block: refused.into_bytes(),
            block_type: Uuid::nil().into_bytes(),
            parent: be_sdk_api::BlockLocation::Block(Uuid::now_v7().into_bytes()),
            defined: Vec::new(),
            content: None,
            owner_only: false,
        },
    );
    tell(&mut instances, ShellMessage::FocusBlockChild(refused.into_bytes()));
    assert!(
        !instances.child_focus_pending(INSTANCE),
        "no child will show a block the host would not make, so the keys typed are not held"
    );
}
