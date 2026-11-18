use super::*;

use be_type_example_counter::CounterContent;

/// What a pass sent the app of `INSTANCE`'s opening: each close, and each opening's kind.
fn opened_and_closed(next: &NextScreens) -> Vec<&'static str> {
    next.opened
        .iter()
        .filter_map(|message| match message {
            Outbound::Close(instance) if *instance == INSTANCE => Some("close"),
            Outbound::Open(opening) if opening.instance == INSTANCE => Some(match &opening.kind {
                OpeningKind::Block(_) => "open block",
                OpeningKind::Join(_) => "open join",
                _ => "open other",
            }),
            _ => None,
        })
        .collect()
}

/// An editor open on a block whose access falls to Join is closed at the app and opened again
/// as a Join member (`opening-kind.join`), given no content; once the access rises again it is
/// closed and opened as an ordinary editor, its content attached again
/// (guides/apps/host_services.md, "Host components"). Each change opens it once.
#[test]
fn an_editor_whose_access_crosses_join_opens_again_as_the_other_kind() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, _) = saved_counter();
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    assert_eq!(
        opened_and_closed(&next_screens(&mut instances)),
        ["open block"]
    );
    assert!(instances.entries[&INSTANCE].content.is_some());

    be_host_stack::set_access_seen(block, be_block_graph::Access::Join);
    assert_eq!(
        opened_and_closed(&next_screens(&mut instances)),
        ["close", "open join"],
        "closed, then opened once as a Join member"
    );
    let entry = &instances.entries[&INSTANCE];
    assert!(
        entry.joined && entry.content.is_none(),
        "a Join member holds no content"
    );
    assert!(
        opened_and_closed(&next_screens(&mut instances)).is_empty(),
        "nothing more while the access stays"
    );

    be_host_stack::set_access_seen(block, be_block_graph::Access::Manage);
    assert_eq!(
        opened_and_closed(&next_screens(&mut instances)),
        ["close", "open block"],
        "closed, then opened once as an editor"
    );
    let entry = &instances.entries[&INSTANCE];
    assert!(
        !entry.joined && entry.content.is_some(),
        "its content is attached again"
    );
}
