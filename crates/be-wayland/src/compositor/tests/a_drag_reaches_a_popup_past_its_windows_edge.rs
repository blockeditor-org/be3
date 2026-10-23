use super::*;
use crate::selection::client;

#[test]
fn a_drag_reaches_a_popup_past_its_windows_edge() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.data_device();
    let (window, id) = harness.open();
    let shown = harness.window(id);
    let start = shown.center();

    harness.frame(vec![
        Input::PointerMoved(start),
        Input::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.settle();
    let serial = harness
        .client
        .received
        .serial
        .expect("the press carried a serial");
    let menu = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (10, 280),
        (150, 200),
    );
    harness.client.start_drag(&device, &window.surface, serial);
    harness.settle();

    let below = pos2(shown.min.x + 50.0, shown.max.y + 100.0);
    harness.frame(vec![Input::PointerMoved(pos2(below.x, below.y - 20.0))]);
    harness.frame(vec![Input::PointerMoved(below)]);
    harness.settle();
    assert_eq!(
        client::drop_target().as_ref(),
        Some(&menu.surface),
        "the drag enters the popup where it reaches past its window"
    );
}
