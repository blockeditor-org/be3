use be_sdk_api::{ParentView, Pos2, Vec2, ViewId, ViewPlacement, ViewportMetrics};

use super::*;

/// Editor `editor`'s main view, screen `screen`.
fn screen(editor: u64, screen: u64) -> ScreenRequest {
    ScreenRequest {
        screen: ScreenId(screen),
        instance: EditorInstanceId(editor),
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: Vec2 { x: 40.0, y: 20.0 },
            visible: Pos2::ZERO,
            pixel_width: 40,
            pixel_height: 20,
            scale_factor: 1.0,
            raster: 1.0,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
        fit: false,
        origin: None,
    }
}

#[test]
fn two_editors_asking_in_turn_are_shown_their_screens_once() {
    // Two editors of one app each ask for their screens at every settle, in whichever order
    // their places were followed: the screens the app is asked for come out the same either
    // way, so `Embedded::show`, which sends `Screens` only when they changed, sends them once.
    let (a, b) = (screen(1, 257), screen(2, 513));
    let first = with_editors_screens(Vec::new(), EditorInstanceId(1), vec![a.clone()]);
    let first = with_editors_screens(first, EditorInstanceId(2), vec![b.clone()]);
    let again = with_editors_screens(first.clone(), EditorInstanceId(2), vec![b.clone()]);
    let again = with_editors_screens(again, EditorInstanceId(1), vec![a.clone()]);
    assert_eq!(first, vec![a.clone(), b.clone()]);
    assert_eq!(
        again, first,
        "asked in the other order, the app is asked for nothing new"
    );

    // An editor that asks for other screens is a change, wherever it falls in the order.
    let mut moved = a.clone();
    moved.metrics.pixel_width = 80;
    let changed = with_editors_screens(again, EditorInstanceId(1), vec![moved.clone()]);
    assert_eq!(changed, vec![moved, b]);
}
