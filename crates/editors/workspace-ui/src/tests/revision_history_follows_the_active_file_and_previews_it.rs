use super::*;
use block_editor_beui::{ChildMode, Revision};
use block_ui_test::{HostCall, Reply};

fn listing(
    fixture: &mut Fixture,
    block: Uuid,
) -> Reply<Result<Vec<Revision>, block_editor_beui::Failure>> {
    fixture
        .test
        .take_requests()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            HostCall::ListRevisions {
                reply,
                block: asked,
            } if asked == block.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("the active file requests its history")
}

#[test]
fn revision_history_follows_the_active_file_and_previews_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
    show(&mut fixture, opened, None);
    let request = listing(&mut fixture, opened);
    fixture.test.answer(
        request,
        Ok(vec![
            Revision {
                commit: [1; 32],
                time: 1_800_000_000_000,
                current: false,
            },
            Revision {
                commit: [2; 32],
                time: 1_800_000_001_000,
                current: true,
            },
        ]),
    );
    fixture.settle();
    fixture.show_history();
    assert!(fixture.says("Current"));
    fixture
        .test
        .snapshot("revision_history_lists_the_active_files_revisions");
    fixture
        .test
        .click(format!("workspace.revision.{}", "01".repeat(32)));
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PreviewRevision {
                reply,
                commit: [1, ..],
                ..
            } => Some(reply),
            _ => None,
        })
        .expect("selecting asks for a preview");
    let preview = Uuid::new_v4();
    fixture.test.answer(request, Ok(preview.into_bytes()));
    fixture.settle();
    assert!(fixture.shown().contains(&preview));
    assert!(!fixture.shown().contains(&opened));
    assert!(
        fixture
            .test
            .children()
            .iter()
            .any(
                |child| child.content.block_id() == Some(preview.into_bytes())
                    && child.mode == ChildMode::Live
            )
    );
    fixture
        .test
        .snapshot("a_revision_is_previewed_with_restore_and_back_controls");
    fixture.test.click("workspace.revision.restore");
    fixture.test.click("workspace.revision.confirm");
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::RestoreRevision {
                reply,
                block,
                commit,
            } if block == opened.into_bytes() && commit == [1; 32] => Some(reply),
            _ => None,
        })
        .expect("restore names the source block and revision");
    fixture.test.answer(
        request,
        Err(block_editor_beui::Failure::Failed(
            "Could not restore".to_owned(),
        )),
    );
    fixture.settle();
    assert!(
        fixture.shown().contains(&preview),
        "failure preserves the preview"
    );
    fixture.test.click("workspace.revision.current");
    assert!(fixture.shown().contains(&opened));
    let second = Uuid::new_v4();
    show(&mut fixture, second, None);
    let request = listing(&mut fixture, second);
    fixture.test.answer(request, Ok(Vec::new()));
    fixture.settle();
    assert!(fixture.says("No saved revisions yet."));
}
