use block_plugin_api::{RegionRef, Size, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.popups";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn open(window: u64, opener: EditorRegion) -> WindowRequest {
    let opener = RegionRef {
        instance: INSTANCE,
        region: opener,
    };
    WindowRequest::Open(WindowOpen {
        window,
        opener,
        placement: WindowPlacement::Over(WindowOver {
            region: opener,
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
    let frame = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Frame);
    let first = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Window(1));
    let second = overlaid(PLUGIN.to_owned(), INSTANCE, EditorRegion::Window(2));
    block_app_plugin_host::request_windows(
        PLUGIN,
        vec![
            open(1, EditorRegion::Frame),
            open(2, EditorRegion::Window(1)),
            open(3, EditorRegion::Window(2)),
            open(4, EditorRegion::Frame),
        ],
    );
    apply();
    assert_eq!(frame.get_untracked(), [1, 4]);
    assert_eq!(
        first.get_untracked(),
        [2],
        "a popup's own popup shows inside it"
    );
    assert_eq!(second.get_untracked(), [3]);

    dismiss(&FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: 2,
    });
    assert_eq!(
        frame.get_untracked(),
        [1, 4],
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
        window: 1,
    });
    assert_eq!(frame.get_untracked(), [4]);
}
