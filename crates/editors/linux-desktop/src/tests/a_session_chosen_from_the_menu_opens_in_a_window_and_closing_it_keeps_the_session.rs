use super::*;

#[test]
fn a_session_chosen_from_the_menu_opens_in_a_window_and_closing_it_keeps_the_session() {
    let (mut fixture, profiles) = Fixture::new().with_sessions(&[
        ("Laptop", WORKSPACE_EDITOR),
        ("Desk", WORKSPACE_EDITOR),
        ("Other desktop", LINUX_DESKTOP_EDITOR),
    ]);
    fixture.settle();

    fixture.test.click("desktop.sessions");
    fixture.settle();
    assert!(fixture.says("Laptop") && fixture.says("Desk"));
    assert!(
        !fixture.says("Other desktop"),
        "a desktop is not offered as a session to open"
    );

    fixture.click_text("Laptop");
    let placed = fixture.placed_blocks();
    let [(block, view, rect)] = placed[..] else {
        panic!("the session opens, and only it: {placed:?}");
    };
    assert_eq!(block, profiles[0]);
    assert_eq!(view, Some(profiles[0]), "the session is its own view");
    assert!(
        rect.min.x > 0.0 && rect.min.y > 0.0,
        "the session floats in a window, at {rect:?}"
    );

    fixture.close_a_tab();
    assert!(fixture.placed_blocks().is_empty(), "the window closes");
    assert_eq!(
        fixture
            .test
            .store()
            .block(profiles[0])
            .map(|info| info.parent),
        Some(BlockParent::Block(fixture.settings)),
        "closing the window keeps the session where it was"
    );
}
