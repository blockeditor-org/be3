use block_plugin_api::{Size, ViewRef, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.desktop";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const TAB: View = View::dock(ViewId(3));

fn open(window: u64, opener: View, at: Pos2) -> WindowRequest {
    let opener = ViewRef {
        instance: INSTANCE,
        view: opener.id,
    };
    WindowRequest::Open(WindowOpen {
        view: ViewId(window),
        screen: block_plugin_api::ScreenId::opened(window),
        opener,
        placement: WindowPlacement::Over(WindowOver { view: opener, at }),
        size: Size {
            width: 100.0,
            height: 50.0,
        },
    })
}

#[test]
fn a_window_opened_from_a_dock_tab_is_hosted_by_that_tab() {
    let _root = beui::reactive::RootScope::new();
    install();
    let tab = overlaid(PLUGIN.to_owned(), INSTANCE, TAB);
    let other_tab = overlaid(PLUGIN.to_owned(), INSTANCE, View::dock(ViewId(4)));
    let frame = overlaid(PLUGIN.to_owned(), INSTANCE, View::MAIN);
    let within = overlaid(PLUGIN.to_owned(), INSTANCE, View::window(ViewId(1)));
    show_view(PLUGIN, INSTANCE, TAB);
    note_origin(PLUGIN, INSTANCE, TAB.id, pos2(200.0, 100.0));
    block_app_plugin_host::request_windows(
        PLUGIN,
        vec![
            open(1, TAB, pos2(10.0, 20.0)),
            open(2, View::window(ViewId(1)), pos2(5.0, 5.0)),
            open(3, View::MAIN, pos2(0.0, 0.0)),
        ],
    );
    apply();
    assert_eq!(tab.get_untracked(), [ViewId(1)]);
    assert_eq!(
        within.get_untracked(),
        [ViewId(2)],
        "a window opened from the tab's window shows inside that one"
    );
    assert_eq!(frame.get_untracked(), [ViewId(3)]);
    assert!(other_tab.get_untracked().is_empty());
    let rect = rect_of(FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: ViewId(1),
    });
    assert_eq!(
        rect.get_untracked().map(|rect| rect.min),
        Some(pos2(210.0, 120.0))
    );
}
