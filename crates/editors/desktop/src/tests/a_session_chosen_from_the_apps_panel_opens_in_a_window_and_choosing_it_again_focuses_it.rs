use block_editor_beui::beui::Pos2;

use super::*;

fn front_block_at(fixture: &Fixture, at: Pos2) -> Option<Uuid> {
    fixture
        .test
        .children()
        .iter()
        .rev()
        .filter(|placement| placement.clip.contains_half_open(at))
        .find_map(|placement| match placement.content {
            ChildContent::Block { block_id, .. } => Some(Uuid::from_bytes(block_id)),
            _ => None,
        })
}

#[test]
fn a_session_chosen_from_the_apps_panel_opens_in_a_window_and_choosing_it_again_focuses_it() {
    let (mut fixture, sessions) = Fixture::new().with_sessions(&[
        ("Laptop", WORKSPACE_EDITOR),
        ("Desk", WORKSPACE_EDITOR),
        ("Other desktop", DESKTOP_EDITOR),
    ]);
    fixture.settle();

    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(fixture.says("Laptop") && fixture.says("Desk"));
    assert!(
        !fixture.says("Other desktop"),
        "a desktop session is not offered as a session to open"
    );

    fixture
        .test
        .click(&format!("desktop.apps.sessions.{}", sessions[0]));
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(block, view, rect)] = placed[..] else {
        panic!("the session opens, and only it: {placed:?}");
    };
    assert_eq!(block, sessions[0]);
    assert_eq!(view, Some(sessions[0]), "the session is its own view");
    assert!(
        rect.min.x > 0.0 && rect.min.y > 0.0,
        "the session floats in a window, at {rect:?}"
    );

    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture
        .test
        .click(&format!("desktop.apps.sessions.{}", sessions[1]));
    fixture.settle();
    assert_eq!(fixture.placed_blocks().len(), 2, "a second session opens");
    let overlap = fixture
        .placed_blocks()
        .iter()
        .map(|(_, _, rect)| *rect)
        .reduce(|first, second| first.intersect(second))
        .expect("both sessions are placed");
    assert_eq!(
        front_block_at(&fixture, overlap.center()),
        Some(sessions[1]),
        "the session opened last is in front"
    );
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(fixture.says("Open"), "an open session says so");
    fixture.test.snapshot("the_apps_panel_with_open_sessions");
    fixture
        .test
        .click(&format!("desktop.apps.sessions.{}", sessions[0]));
    fixture.settle();
    let placed = fixture.placed_blocks();
    assert_eq!(
        placed.len(),
        2,
        "choosing an open session opens no second window of it: {placed:?}"
    );
    let overlap = placed
        .iter()
        .map(|(_, _, rect)| *rect)
        .reduce(|first, second| first.intersect(second))
        .expect("both sessions are placed");
    assert_eq!(
        front_block_at(&fixture, overlap.center()),
        Some(sessions[0]),
        "its window comes in front of the other session's"
    );

    fixture.close_a_tab();
    fixture.close_a_tab();
    assert!(fixture.placed_blocks().is_empty(), "the windows close");
    assert_eq!(
        fixture
            .test
            .store()
            .block(sessions[0])
            .and_then(|info| info.parent),
        Some(BlockParent::Block(fixture.settings)),
        "closing the window keeps the session where it was"
    );
}
