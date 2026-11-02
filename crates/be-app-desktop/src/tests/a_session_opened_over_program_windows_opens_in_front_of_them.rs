use super::*;

fn front_at(fixture: &Fixture, at: Pos2) -> Option<ChildContent> {
    fixture
        .test
        .children()
        .iter()
        .rev()
        .find(|placement| placement.clip.contains_half_open(at))
        .map(|placement| placement.content.clone())
}

#[test]
fn a_session_opened_over_program_windows_opens_in_front_of_them() {
    let (mut fixture, sessions) = Fixture::new().with_sessions(&[("Laptop", WORKSPACE_EDITOR)]);
    fixture.settle();
    fixture.show_windows(&[host_window(1, false), host_window(2, true)]);
    fixture.test.clear_calls();

    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture
        .test
        .click(&format!("desktop.apps.sessions.{}", sessions[0]));
    fixture.settle();

    let [(block, _, session)] = fixture.placed_blocks()[..] else {
        panic!("the session opens");
    };
    assert_eq!(block, sessions[0]);
    for window in [1, 2] {
        let program = fixture.window_rect(window).expect("the program's window shows");
        let overlap = session.intersect(program);
        assert!(overlap.is_positive(), "the windows overlap: {session:?} {program:?}");
        assert!(
            matches!(
                front_at(&fixture, overlap.center()),
                Some(ChildContent::Block { block_id, .. }) if Uuid::from_bytes(block_id) == block
            ),
            "the session opens in front of window {window}"
        );
    }
    assert!(
        fixture.test.document().find_test_id("desktop.apps.panel").is_none(),
        "choosing the session closes the panel"
    );
    fixture.test.snapshot("a_session_opened_over_program_windows");
}
