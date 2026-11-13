use be_sdk_beui::BlockQuery;

use crate::link::Subject;

use super::*;

struct Quiet;

#[component]
impl BeuiRoot for Quiet {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <List spacing=Space::Sm />
            </Root>
        }
    }
}

fn artifact(block: Uuid, summary: &str) -> be_sdk_api::ArtifactState {
    be_sdk_api::ArtifactState {
        block_id: block.into_bytes(),
        source_type: Uuid::nil().into_bytes(),
        source: Uuid::nil().into_bytes(),
        summary: Some(summary.to_owned()),
        error: None,
        regenerating: false,
    }
}

#[test]
fn narrowing_one_kind_of_block_watch_keeps_the_others() {
    let mut test: BeuiTest<Quiet> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.run();
    let (folder, child, history, kept, dropped) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    test.store().add_block(BlockInfo::new(
        child,
        Uuid::new_v4(),
        BlockParent::Block(folder),
    ));
    let host = test.host().clone();
    let listing = host.graph().watch(BlockQuery::Children(folder));
    host.watch_history([history]);
    host.watch_artifacts([kept, dropped]);
    test.run();
    let before = test.block_watches();
    let op_of = |watches: &[(be_sdk_api::ops::OpId, Subject)], subject: Subject| {
        watches
            .iter()
            .find(|(_, watched)| *watched == subject)
            .map(|(op, _)| *op)
    };
    for subject in [
        Subject::Blocks(BlockQuery::Children(folder)),
        Subject::History(history),
        Subject::Artifact(kept),
        Subject::Artifact(dropped),
    ] {
        let held = before.iter().filter(|(_, watched)| *watched == subject);
        assert_eq!(held.count(), 1, "one watch of {subject:?}: {before:?}");
    }

    test.set_histories([(
        history,
        be_sdk_beui::BlockHistory {
            can_undo: true,
            ..Default::default()
        },
    )]);
    test.set_artifact(kept, Some(artifact(kept, "kept")));
    test.set_artifact(dropped, Some(artifact(dropped, "dropped")));
    test.run();

    host.watch_artifacts([kept]);
    test.run();
    let after = test.block_watches();
    assert_eq!(
        after.len(),
        before.len() - 1,
        "only the dropped artifact's watch closed: {after:?}"
    );
    assert_eq!(op_of(&after, Subject::Artifact(dropped)), None);
    for subject in [
        Subject::Blocks(BlockQuery::Children(folder)),
        Subject::History(history),
        Subject::Artifact(kept),
    ] {
        assert!(
            op_of(&after, subject).is_some(),
            "{subject:?} is still watched"
        );
        assert_eq!(
            op_of(&after, subject),
            op_of(&before, subject),
            "{subject:?} keeps its watch"
        );
    }

    let listed = listing.read().expect("the folder was listed");
    assert_eq!(
        listed.iter().map(|block| block.id).collect::<Vec<_>>(),
        [child]
    );
    assert!(
        host.history(history).can_undo,
        "the history reached its block"
    );
    assert!(
        !host.history(kept).can_undo,
        "a history lands only on its own block"
    );
    assert_eq!(
        host.artifact(kept)
            .and_then(|state| state.summary)
            .as_deref(),
        Some("kept"),
        "the artifact's state is kept once the other closes"
    );
    assert!(
        host.artifact(history).is_none(),
        "an artifact state lands only on its own block"
    );
}
