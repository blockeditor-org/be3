use block_plugin_api::{RegionRef, Size, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.desktop";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const TAB: EditorRegion = EditorRegion::Tab(3);

fn open(window: u64, opener: EditorRegion, at: Pos2) -> WindowRequest {
    let opener = RegionRef {
        instance: INSTANCE,
        region: opener,
    };
    WindowRequest::Open(WindowOpen {
        window,
        opener,
        placement: WindowPlacement::Over(WindowOver { region: opener, at }),
        size: Size {
            width: 100.0,
            height: 50.0,
        },
        role: WindowRole::Popup,
    })
}

#[test]
fn a_window_opened_from_a_dock_tab_is_hosted_by_that_tab() {
    let _root = beui::reactive::RootScope::new();
    install();
    let tab = overlaid(PLUGIN.to_owned(), INSTANCE, TAB);
    let other_tab = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Tab(4));
    let frame = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Frame);
    let within = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Window(1));
    note_origin(PLUGIN, INSTANCE, TAB, pos2(200.0, 100.0));
    crate::plugin_host::request_windows(
        PLUGIN,
        vec![
            open(1, TAB, pos2(10.0, 20.0)),
            open(2, EditorRegion::Window(1), pos2(5.0, 5.0)),
            open(3, EditorRegion::Frame, pos2(0.0, 0.0)),
        ],
    );
    apply();
    assert_eq!(tab.get_untracked(), [1]);
    assert_eq!(
        within.get_untracked(),
        [2],
        "a window opened from the tab's window shows inside that one"
    );
    assert_eq!(frame.get_untracked(), [3]);
    assert!(other_tab.get_untracked().is_empty());
    let rect = rect_of(FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: 1,
    });
    assert_eq!(
        rect.get_untracked().map(|rect| rect.min),
        Some(pos2(210.0, 120.0))
    );
}
