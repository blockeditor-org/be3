use super::*;

#[test]
fn a_window_the_host_makes_fullscreen_covers_the_workspace_until_it_leaves() {
    let (mut fixture, _) = editor();
    fixture.show_windows(&[window(3, "Videos", None)]);
    fixture.settle();
    let docked = placed_windows(&fixture)[0].1;

    let mut fullscreen = window(3, "Videos", None);
    fullscreen.fullscreen = true;
    fixture.show_windows(&[fullscreen]);
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture),
        vec![(
            3,
            Rect::from_min_size(be_sdk_beui::pos2(0.0, 0.0), be_sdk_beui::vec2(800.0, 600.0))
        )],
        "the window its program makes full screen covers the workspace, tab bars and all"
    );
    assert!(
        fixture
            .test
            .children()
            .iter()
            .all(|placement| placement.fullscreen),
        "and is placed full screen, as its program asked"
    );

    fixture.show_windows(&[window(3, "Videos", None)]);
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture),
        vec![(3, docked)],
        "leaving puts the window back in its tab"
    );
}
