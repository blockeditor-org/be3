use super::*;
use wayland_protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1;

#[test]
fn activating_a_window_with_a_modal_dialog_raises_the_dialog() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let (parent, parent_id) = harness.open();
    let (dialog, dialog_id) = harness.open();
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    let dialogs: XdgWmDialogV1 = harness.client.bind("xdg_wm_dialog_v1", 1);
    dialogs
        .get_xdg_dialog(toplevel, &harness.client.handle, ())
        .set_modal();
    harness.settle();
    let windows = harness.compositor().windows();
    let (parent_raised, dialog_raised) = (windows.raised(parent_id), windows.raised(dialog_id));

    windows.activate(parent_id);
    harness.settle();

    assert_eq!(
        (windows.raised(parent_id), windows.raised(dialog_id)),
        (parent_raised, dialog_raised + 1),
        "the dialog comes forward in its parent's place"
    );
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&dialog.surface),
        "and takes the keyboard"
    );
}
