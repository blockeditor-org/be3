use super::*;
use crate::test_client::Seen;
use wayland_protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1;

#[test]
fn a_press_held_when_a_dialog_turns_modal_is_still_released() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (parent, parent_id) = harness.open();
    let (dialog, _) = harness.open();
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    harness.settle();

    let at = harness.window(parent_id).center();
    let button = |pressed| Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
    };
    harness.frame(vec![Input::PointerMoved(at), button(true)]);
    harness.settle();

    let dialogs: XdgWmDialogV1 = harness.client.bind("xdg_wm_dialog_v1", 1);
    dialogs
        .get_xdg_dialog(toplevel, &harness.client.handle, ())
        .set_modal();
    harness.settle();
    harness.frame(vec![button(false)]);
    harness.settle();

    assert!(
        harness
            .client
            .received
            .seen
            .contains(&Seen::Button(parent.surface.clone(), false)),
        "the parent hears the release of the press it was given"
    );
    assert!(
        !harness.compositor().server().state.buttons_held(),
        "no button is left held"
    );
}
