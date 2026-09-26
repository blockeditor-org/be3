use super::*;

use block_plugin_api::{ConflictSide, VersionCommand};

#[test]
fn a_conflicting_edit_keeps_every_side_until_one_is_chosen() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    let second = second_checkout(&versioned);
    let note = copy_of(versioned.note, second);
    version_since(
        versioned.checkout,
        be_block::CheckoutContent::CONTENT_TYPE,
        None,
    );
    version_since(second, be_block::CheckoutContent::CONTENT_TYPE, None);
    hold(versioned.note, be_block::TextContent::CONTENT_TYPE);
    hold(note, be_block::TextContent::CONTENT_TYPE);
    wait_until("opened both notes", |shared| {
        text_of(shared, versioned.note).is_some() && text_of(shared, note).is_some()
    });

    type_text(versioned.note, 5, " there");
    type_text(note, 5, " world");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Commit {
            message: "There".into(),
        },
    );
    wait_until("saw the other checkout fall behind", |shared| {
        version_status(shared, second).is_some_and(|status| status.behind)
    });
    version(second, VersionCommand::Update);
    wait_until("recorded the conflict", |shared| {
        settled(shared, second)
            && checkout_state(shared, second).is_some_and(|state| state.conflicts.len() == 1)
    });

    let conflict = with_shared(|shared| checkout_state(shared, second))
        .flatten()
        .map(|state| (*state.conflicts[0]).clone())
        .expect("the conflict is recorded");
    assert_eq!(conflict.block, note);
    assert_eq!(conflict.kind, be_block::ConflictKind::Content);
    let sides = [conflict.base, conflict.ours, conflict.theirs]
        .map(|side| side.expect("every side was kept"));
    for side in sides {
        hold(side, be_block::TextContent::CONTENT_TYPE);
    }
    wait_until("opened every side", |shared| {
        sides.iter().all(|side| text_of(shared, *side).is_some())
    });
    assert_eq!(
        with_shared(|shared| sides.map(|side| text_of(shared, side))),
        Some([
            Some("hello\n".to_owned()),
            Some("hello world\n".to_owned()),
            Some("hello there\n".to_owned()),
        ])
    );

    version(
        second,
        VersionCommand::Resolve {
            block_id: note.into_bytes(),
            take: ConflictSide::Theirs,
        },
    );
    wait_until("took their side", |shared| {
        text_of(shared, note).as_deref() == Some("hello there\n")
            && checkout_state(shared, second).is_some_and(|state| state.conflicts.is_empty())
    });
    for side in sides {
        assert_eq!(
            node(side).map(|node| node.parent),
            Some(be_graph::BlockParent::Detached)
        );
    }
}
