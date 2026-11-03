use be_sdk_api::{Size, ViewRef, WindowOpen, WindowOver};

use super::*;

const APP: &str = "test.raise";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn open(window: u64, opener: View) -> WindowRequest {
    let opener = ViewRef {
        instance: INSTANCE,
        view: opener.id,
    };
    WindowRequest::Open(WindowOpen {
        view: ViewId(window),
        screen: be_sdk_api::ScreenId::opened(window),
        opener,
        placement: WindowPlacement::Over(WindowOver {
            view: opener,
            at: pos2(4.0, 4.0),
        }),
        size: Size {
            width: 80.0,
            height: 40.0,
        },
        level: be_sdk_api::WindowLevel::Normal,
    })
}

fn order() -> Vec<u64> {
    floated().with_untracked(|windows| windows.iter().map(|window| window.key.window.0).collect())
}

#[test]
fn a_raised_window_brings_the_windows_opened_over_it_along() {
    let _root = beui::reactive::RootScope::new();
    install();
    be_host_runtime::request_windows(
        APP,
        vec![
            open(1, View::MAIN),
            open(2, View::window(ViewId(1))),
            open(3, View::MAIN),
        ],
    );
    apply();
    assert_eq!(order(), [1, 2, 3]);

    be_host_runtime::request_windows(APP, vec![WindowRequest::Raise(ViewId(1))]);
    apply();
    assert_eq!(
        order(),
        [3, 1, 2],
        "the dialog window 2 opened over window 1 stays in front of it"
    );

    be_host_runtime::request_windows(APP, vec![WindowRequest::Raise(ViewId(3))]);
    apply();
    assert_eq!(order(), [1, 2, 3]);
}
