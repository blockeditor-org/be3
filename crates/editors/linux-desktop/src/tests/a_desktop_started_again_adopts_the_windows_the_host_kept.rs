use block_editor_beui::beui::{Pos2, Vec2, vec2};

use super::*;

const SCREEN: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 600.0));

fn placed(fixture: &Fixture, window: u64) -> Option<Rect> {
    fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content == ChildContent::Window(HostWindowId(window)))
        .map(|placement| placement.rect)
}

fn kept(fullscreen: bool) -> Vec<HostWindow> {
    vec![
        HostWindow {
            fullscreen: fullscreen.then_some(SCREEN),
            ..host_window(1, false)
        },
        host_window(2, true),
        HostWindow {
            parent: Some(HostWindowId(2)),
            title: "Save as".to_owned(),
            ..host_window(3, false)
        },
    ]
}

#[test]
fn a_desktop_started_again_adopts_the_windows_the_host_kept() {
    let mut first = Fixture::new();
    first.settle();
    first.test.set_watched::<WindowsWatch>(&kept(false));
    first.settle();
    let opened = placed(&first, 2).expect("the desktop shows the window");
    let tab = first
        .test
        .point_of(&format!("dock.tab.{}.close", window_tab(2)))
        - Vec2::new(60.0, 0.0);
    first.test.drag(tab, tab + Vec2::new(120.0, 80.0));
    first.settle();
    let moved = placed(&first, 2).expect("the window is still shown");
    assert_ne!(moved.min, opened.min, "dragging its tab moves the window");
    let saved: EditorViewContent = first.test.content(None);

    let mut second = Fixture::holding(saved);
    second.test.set_watched::<WindowsWatch>(&kept(true));
    second.settle();

    assert_eq!(
        placed(&second, 2).map(|rect| rect.min),
        Some(moved.min),
        "a window the saved layout placed returns to its place"
    );
    assert!(
        second.says("Program 2"),
        "and has its tab, as if it had just opened"
    );
    assert_eq!(
        placed(&second, 1).map(|rect| rect.size()),
        Some(vec2(SCREEN.width(), SCREEN.height())),
        "a fullscreen window stays fullscreen"
    );
    assert!(
        placed(&second, 3).is_some() && second.says("Save as"),
        "a dialog is shown over its parent again"
    );
    assert!(
        second
            .test
            .take_host_calls(block_ui_test::requested::window)
            .iter()
            .all(|action| !matches!(
                action,
                WindowAction::Close(_)
                    | WindowAction::Fullscreen {
                        fullscreen: false,
                        ..
                    }
            )),
        "adopting the windows closes none and takes none out of fullscreen"
    );
}
