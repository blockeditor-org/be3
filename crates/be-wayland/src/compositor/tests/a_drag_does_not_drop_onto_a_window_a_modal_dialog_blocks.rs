use super::*;
use crate::selection::client;
use wayland_protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1;

#[test]
fn a_drag_does_not_drop_onto_a_window_a_modal_dialog_blocks() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.data_device();
    let (from, first) = harness.open();
    let (parent, second) = harness.open();
    let (dialog, _) = harness.open();
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    let dialogs: XdgWmDialogV1 = harness.client.bind("xdg_wm_dialog_v1", 1);
    dialogs
        .get_xdg_dialog(toplevel, &harness.client.handle, ())
        .set_modal();
    harness.settle();
    let start = harness.window(first).center();
    let end = harness.window(second).center();

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
    harness.client.start_drag(&device, &from.surface, serial);
    harness.settle();

    harness.frame(vec![Input::PointerMoved(end)]);
    harness.settle();
    assert_ne!(
        client::drop_target().as_ref(),
        Some(&parent.surface),
        "a window behind a modal dialog takes no drag"
    );

    harness.frame(vec![Input::PointerButton {
        pos: end,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.settle();
    assert_eq!(client::dropped_on(), None);
}
