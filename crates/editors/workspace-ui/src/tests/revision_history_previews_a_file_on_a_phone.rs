use super::*;
use block_editor_beui::Revision;
use block_ui_test::HostCall;

#[test]
fn revision_history_previews_a_file_on_a_phone() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 720.0)));
    show(&mut fixture, opened, None);
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListRevisions { reply, block } if block == opened.into_bytes() => Some(reply),
            _ => None,
        })
        .unwrap();
    fixture.test.answer(
        request,
        Ok(vec![Revision {
            commit: [4; 32],
            time: 1_800_000_000_000,
            current: true,
        }]),
    );
    fixture.settle();
    fixture.test.click("dock.switch");
    fixture.test.settle();
    fixture.test.click(format!(
        "dock.switcher.tab.{}",
        block_shell::REVISIONS.value()
    ));
    fixture.test.settle();
    fixture
        .test
        .click(format!("workspace.revision.{}", "04".repeat(32)));
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PreviewRevision { reply, .. } => Some(reply),
            _ => None,
        })
        .unwrap();
    let preview = Uuid::new_v4();
    fixture.test.answer(request, Ok(preview.into_bytes()));
    fixture.settle();
    assert!(
        fixture.shown().contains(&preview),
        "selecting a revision returns to the document on a phone"
    );
    fixture.test.click("workspace.revision.restore");
    fixture.test.settle();
    fixture
        .test
        .snapshot("revision_history_confirms_a_restore_on_a_phone");
    fixture.test.click("workspace.revision.cancel");
    fixture.test.click("workspace.revision.current");
    assert!(fixture.shown().contains(&opened));
}
