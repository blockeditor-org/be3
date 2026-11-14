use super::*;

/// A block the server will not make (its name is too large for it) leaves the files for the
/// unsent edits, where it can be restored or discarded; its open replica does not try to join
/// a session the server cannot have.
#[test]
fn a_block_the_server_refuses_to_make_leaves_the_files_for_the_unsent_edits() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("n".repeat(70 * 1024)),
        None,
        false,
    );
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);

    wait_until("set the refused block aside", |shared| {
        shared
            .unsent
            .iter()
            .any(|unsent| unsent.block == block && unsent.refused.is_some())
            && shared.graph.get(block).is_none()
    });
    flush_and_send();
    assert!(
        crate::wait_until(|shared| Some(shared.error.clone())).is_none(),
        "the refused block's replica tried to join a session the server does not have"
    );
    assert_eq!(
        count_of(block),
        Some(2),
        "what was made of it is still shown"
    );
    let said = crate::wait_until(|shared| Some(shared.sync_error.clone())).unwrap_or_default();
    assert!(
        said.contains(crate::refusal(be_block_protocol::ErrorCode::InvalidRequest))
            && !said.contains("InvalidRequest"),
        "the refusal is said in words, its code kept out: {said}"
    );
}
