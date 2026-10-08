use super::*;

#[test]
fn a_window_the_host_makes_fullscreen_covers_the_workspace_until_it_leaves() {
    let (mut fixture, _) = editor();
    fixture.host.set_windows(vec![window(3, "Player", None)]);
    fixture.settle();
    let docked = placed_windows(&fixture)[0].1;

    let screen = block_plugin_api::ChildRect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let mut fullscreen = window(3, "Player", None);
    fullscreen.fullscreen = Some(screen);
    fixture.host.set_windows(vec![fullscreen]);
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
        !fixture.test.sent().iter().any(|message| matches!(
            message,
            block_plugin_api::EditorMessage::Linux {
                message: block_plugin_api::LinuxMessage::FullscreenWindow { .. },
                ..
            }
        )),
        "a fullscreen the host asked for is not asked for back"
    );

    fixture.host.set_windows(vec![window(3, "Player", None)]);
    fixture.settle();
    assert_eq!(
        placed_windows(&fixture),
        vec![(3, docked)],
        "leaving puts the window back in its tab"
    );
}
