use block_plugin_api::{Size, ViewRef, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.raise";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn open(window: u64, opener: View) -> WindowRequest {
    let opener = ViewRef {
        instance: INSTANCE,
        view: opener.id,
    };
    WindowRequest::Open(WindowOpen {
        view: ViewId(window),
        screen: block_plugin_api::ScreenId::opened(window),
        opener,
        placement: WindowPlacement::Over(WindowOver {
            view: opener,
            at: pos2(4.0, 4.0),
        }),
        size: Size {
            width: 80.0,
            height: 40.0,
        },
        role: WindowRole::Normal,
    })
}

fn order() -> Vec<u64> {
    floated().with_untracked(|windows| windows.iter().map(|window| window.key.window.0).collect())
}

#[test]
fn a_raised_window_brings_the_windows_opened_over_it_along() {
    let _root = beui::reactive::RootScope::new();
    install();
    block_app_plugin_host::request_windows(
        PLUGIN,
        vec![
            open(1, View::MAIN),
            open(2, View::window(ViewId(1))),
            open(3, View::MAIN),
        ],
    );
    apply();
    assert_eq!(order(), [1, 2, 3]);

    block_app_plugin_host::request_windows(PLUGIN, vec![WindowRequest::Raise(ViewId(1))]);
    apply();
    assert_eq!(
        order(),
        [3, 1, 2],
        "the dialog window 2 opened over window 1 stays in front of it"
    );

    block_app_plugin_host::request_windows(PLUGIN, vec![WindowRequest::Raise(ViewId(3))]);
    apply();
    assert_eq!(order(), [1, 2, 3]);
}
