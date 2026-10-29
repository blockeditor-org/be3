use block_plugin_api::{Size, ViewRef, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.popups";
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
        role: WindowRole::Popup,
    })
}

#[test]
fn a_popup_holds_its_own_popups_and_takes_them_when_dismissed() {
    let _root = beui::reactive::RootScope::new();
    install();
    let frame = overlaid(PLUGIN.to_owned(), INSTANCE, View::MAIN);
    let first = overlaid(PLUGIN.to_owned(), INSTANCE, View::window(ViewId(1)));
    let second = overlaid(PLUGIN.to_owned(), INSTANCE, View::window(ViewId(2)));
    block_app_plugin_host::request_windows(
        PLUGIN,
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
        "a popup's own popup shows inside it"
    );
    assert_eq!(second.get_untracked(), [ViewId(3)]);

    dismiss(&FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: ViewId(2),
    });
    assert_eq!(
        frame.get_untracked(),
        [ViewId(1), ViewId(4)],
        "dismissing a popup's own popup leaves the popup open"
    );
    assert!(second.get_untracked().is_empty());
    assert!(
        first.get_untracked().is_empty(),
        "and closes what it opened"
    );

    dismiss(&FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: ViewId(1),
    });
    assert_eq!(frame.get_untracked(), [ViewId(4)]);
}
