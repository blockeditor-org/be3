use super::*;
use block_editor_beui::beui::{Pos2, Vec2};

#[test]
fn a_window_the_host_makes_fullscreen_covers_the_workspace_until_it_leaves() {
    let (mut fixture, _) = editor();
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(3, "Videos", None)]);
    fixture.settle();
    let docked = placed_windows(&fixture)[0].1;

    let screen = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    let mut fullscreen = window(3, "Videos", None);
    fullscreen.fullscreen = Some(screen);
    fixture.host.set_watched::<WindowsWatch>(&vec![fullscreen]);
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture),
        vec![(
            3,
            Rect::from_min_size(
                block_editor_beui::pos2(0.0, 0.0),
                block_editor_beui::vec2(800.0, 600.0)
            )
        )],
        "the window covers the screen the host named, tab bars and all"
    );
    assert!(
        !fixture
            .test
            .host_calls(block_ui_test::requested::window)
            .iter()
            .any(|action| matches!(action, WindowAction::Fullscreen { .. })),
        "a fullscreen the host asked for is not asked for back"
    );

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(3, "Videos", None)]);
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture),
        vec![(3, docked)],
        "leaving puts the window back in its tab"
    );
}
