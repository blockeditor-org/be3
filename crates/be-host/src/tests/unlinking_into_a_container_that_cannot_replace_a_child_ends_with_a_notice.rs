use be_block_content::BlockContent;

use super::*;

/// A container whose type cannot put a copy in a child's place refuses an unlink once: the copy
/// is dropped with a notice instead of waiting forever, and nothing is duplicated.
#[test]
fn unlinking_into_a_container_that_cannot_replace_a_child_ends_with_a_notice() {
    let state = HostStateStore::open(":memory:").expect("the host state opens");
    let mut core = HostCore::with_state(state).expect("the app starts");
    let (source, container) = (Uuid::new_v4(), Uuid::new_v4());
    core.block_types
        .insert(source, be_block::TagsContent::CONTENT_TYPE);
    core.block_types
        .insert(container, be_block::SettingsContent::CONTENT_TYPE);

    core.queue_copy(source, container);
    assert_eq!(core.pending_copies.len(), 1);
    core.process_pending_copies();

    assert!(
        core.pending_copies.is_empty(),
        "a refused copy is not tried again"
    );
    assert!(
        notices::shown()
            .iter()
            .any(|notice| notice.refusal && notice.message.contains("still links to the original")),
        "the person is told the container still links to the original"
    );
}
