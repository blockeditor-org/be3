use super::*;
use block_editor_beui::{ChildMode, HostReply, HostRequest, Revision};

fn listing(fixture: &mut Fixture, block: Uuid) -> u64 {
    fixture
        .test
        .take_requests()
        .into_iter()
        .rev()
        .find_map(|(id, request)| {
            (request == HostRequest::ListRevisions(block.into_bytes())).then_some(id)
        })
        .expect("the active file requests its history")
}

#[test]
fn revision_history_follows_the_active_file_and_previews_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
    show(&mut fixture, opened, None);
    let request = listing(&mut fixture, opened);
    fixture.test.reply(
        request,
        HostReply::RevisionsListed(Ok(vec![
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
        ])),
    );
    fixture.settle();
    fixture.show_history();
    assert!(fixture.says("Current"));
    fixture
        .test
        .snapshot("revision_history_lists_the_active_files_revisions");
    fixture
        .test
        .click(&format!("workspace.revision.{}", "01".repeat(32)));
    fixture.settle();
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(id, request)| {
            matches!(
                request,
                HostRequest::PreviewRevision {
                    commit: [1, ..],
                    ..
                }
            )
            .then_some(id)
        })
        .expect("selecting asks for a preview");
    let preview = Uuid::new_v4();
    fixture.test.reply(
        request,
        HostReply::RevisionPreviewed(Ok(preview.into_bytes())),
    );
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
    fixture.settle();
    fixture.test.click("workspace.revision.confirm");
    fixture.settle();
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(id, request)| {
            (request
                == HostRequest::RestoreRevision {
                    block_id: opened.into_bytes(),
                    commit: [1; 32],
                })
            .then_some(id)
        })
        .expect("restore names the source block and revision");
    fixture.test.reply(
        request,
        HostReply::RevisionRestored(Err("Could not restore".into())),
    );
    fixture.settle();
    assert!(
        fixture.shown().contains(&preview),
        "failure preserves the preview"
    );
    fixture.test.click("workspace.revision.current");
    fixture.settle();
    assert!(fixture.shown().contains(&opened));
    let second = Uuid::new_v4();
    show(&mut fixture, second, None);
    let request = listing(&mut fixture, second);
    fixture
        .test
        .reply(request, HostReply::RevisionsListed(Ok(Vec::new())));
    fixture.settle();
    assert!(fixture.says("No saved revisions yet."));
}
