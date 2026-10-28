use block_plugin_api::{RegionRef, Size, WindowOpen, WindowOver};

use super::*;

const PLUGIN: &str = "test.raise";
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
        role: WindowRole::Normal,
    })
}

fn order() -> Vec<u64> {
    floated().with_untracked(|windows| windows.iter().map(|window| window.key.window).collect())
}

#[test]
fn a_raised_window_brings_the_windows_opened_over_it_along() {
    let _root = beui::reactive::RootScope::new();
    install();
    request(
        PLUGIN,
        vec![
            open(1, EditorRegion::Frame),
            open(2, EditorRegion::Window(1)),
            open(3, EditorRegion::Frame),
        ],
    );
    apply();
    assert_eq!(order(), [1, 2, 3]);

    request(PLUGIN, vec![WindowRequest::Raise(1)]);
    apply();
    assert_eq!(
        order(),
        [3, 1, 2],
        "the dialog window 2 opened over window 1 stays in front of it"
    );

    request(PLUGIN, vec![WindowRequest::Raise(3)]);
    apply();
    assert_eq!(order(), [1, 2, 3]);
}
