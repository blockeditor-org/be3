use be_sdk_api::{Vec2, ViewRef, WindowOpen, WindowOver};

use super::*;

const APP: &str = "test.windows";
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
        size: Vec2 { x: 80.0, y: 40.0 },
        level: be_sdk_api::WindowLevel::Normal,
    })
}

#[test]
fn a_closed_window_takes_the_windows_it_opened_with_it() {
    let _root = beui::reactive::RootScope::new();
    install();
    let frame = overlaid(APP.to_owned(), INSTANCE, View::MAIN);
    let first = overlaid(APP.to_owned(), INSTANCE, View::window(ViewId(1)));
    let second = overlaid(APP.to_owned(), INSTANCE, View::window(ViewId(2)));
    be_host_runtime::request_windows(
        APP,
        vec![
            open(1, View::MAIN),
            open(2, View::window(ViewId(1))),
            open(3, View::window(ViewId(2))),
            open(4, View::MAIN),
        ],
    );
    apply();
    assert_eq!(frame.get_untracked(), [ViewId(1), ViewId(4)]);
    assert_eq!(
        first.get_untracked(),
        [ViewId(2)],
        "a window's own window shows inside it"
    );
    assert_eq!(second.get_untracked(), [ViewId(3)]);

    dismiss(&FloatedKey {
        app: APP.to_owned(),
        instance: INSTANCE,
        window: ViewId(2),
    });
    assert_eq!(
        frame.get_untracked(),
        [ViewId(1), ViewId(4)],
        "closing a window's own window leaves the window open"
    );
    assert!(second.get_untracked().is_empty());
    assert!(
        first.get_untracked().is_empty(),
        "and closes what it opened"
    );

    dismiss(&FloatedKey {
        app: APP.to_owned(),
        instance: INSTANCE,
        window: ViewId(1),
    });
    assert_eq!(frame.get_untracked(), [ViewId(4)]);
}
