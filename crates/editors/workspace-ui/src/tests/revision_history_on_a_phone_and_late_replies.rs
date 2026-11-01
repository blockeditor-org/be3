use super::*;
use block_editor_beui::Revision;
use block_ui_test::HostCall;

#[test]
fn revision_history_on_a_phone_and_late_replies() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
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
            commit: [3; 32],
            time: 1_800_000_000_000,
            current: true,
        }]),
    );
    fixture.settle();
    fixture.show_history();
    fixture
        .test
        .click(format!("workspace.revision.{}", "03".repeat(32)));
    fixture.settle();
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PreviewRevision { reply, .. } => Some(reply),
            _ => None,
        })
        .unwrap();
    let second = Uuid::new_v4();
    show(&mut fixture, second, None);
    let preview = Uuid::new_v4();
    fixture.test.answer(request, Ok(preview.into_bytes()));
    fixture.settle();
    assert!(!fixture.shown().contains(&preview));
    let released = fixture.test.take_requests_where(|call| match call {
        HostCall::ReleaseRevision { preview, .. } => Ok(preview),
        call => Err(call),
    });
    assert_eq!(released, vec![preview.into_bytes()]);
    fixture.test.set_size(Vec2::new(390.0, 720.0));
    fixture.settle();
    fixture.test.click("dock.switch");
    fixture.settle();
    fixture.test.settle();
    fixture.click_text("History");
    fixture
        .test
        .snapshot("revision_history_is_available_in_the_phone_switcher");
}
