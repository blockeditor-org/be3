use super::*;
use block_editor_beui::{HostReply, HostRequest, Revision};

#[test]
fn revision_history_previews_a_file_on_a_phone() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 720.0)));
    show(&mut fixture, opened, None);
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(id, request)| {
            (request == HostRequest::ListRevisions(opened.into_bytes())).then_some(id)
        })
        .unwrap();
    fixture.test.reply(
        request,
        HostReply::RevisionsListed(Ok(vec![Revision {
            commit: [4; 32],
            time: 1_800_000_000_000,
            current: true,
        }])),
    );
    fixture.settle();
    fixture.test.click("dock.switch");
    fixture.settle();
    fixture.test.settle();
    fixture.test.click(format!(
        "dock.switcher.tab.{}",
        block_shell::REVISIONS.value()
    ));
    fixture.settle();
    fixture.test.settle();
    fixture
        .test
        .click(format!("workspace.revision.{}", "04".repeat(32)));
    fixture.settle();
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(id, request)| {
            matches!(request, HostRequest::PreviewRevision { .. }).then_some(id)
        })
        .unwrap();
    let preview = Uuid::new_v4();
    fixture.test.reply(
        request,
        HostReply::RevisionPreviewed(Ok(preview.into_bytes())),
    );
    fixture.settle();
    assert!(
        fixture.shown().contains(&preview),
        "selecting a revision returns to the document on a phone"
    );
    fixture.test.click("workspace.revision.restore");
    fixture.settle();
    fixture.test.settle();
    fixture
        .test
        .snapshot("revision_history_confirms_a_restore_on_a_phone");
    fixture.test.click("workspace.revision.cancel");
    fixture.settle();
    fixture.test.click("workspace.revision.current");
    fixture.settle();
    assert!(fixture.shown().contains(&opened));
}
