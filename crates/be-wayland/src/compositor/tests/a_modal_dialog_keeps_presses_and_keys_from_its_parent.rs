use super::*;
use crate::test_client::Seen;
use wayland_protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1;

const KEY_A: u32 = 30;

#[test]
fn a_modal_dialog_keeps_presses_and_keys_from_its_parent() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (parent, parent_id) = harness.open();
    let (dialog, dialog_id) = harness.open();
    let dialogs: XdgWmDialogV1 = harness.client.bind("xdg_wm_dialog_v1", 1);
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    let hint = dialogs.get_xdg_dialog(toplevel, &harness.client.handle, ());
    hint.set_modal();
    harness.settle();

    let listed = harness
        .compositor()
        .windows()
        .list()
        .get_untracked()
        .into_iter()
        .find(|info| info.id == dialog_id)
        .expect("the dialog is listed");
    assert_eq!(
        listed.parent,
        Some(parent_id),
        "the dialog names its parent"
    );
    assert!(listed.modal, "the dialog is listed as modal");

    let pressed = |harness: &Harness, surface: &wayland_client::protocol::wl_surface::WlSurface| {
        harness
            .client
            .received
            .seen
            .iter()
            .any(|seen| *seen == Seen::Button(surface.clone(), true))
    };
    harness.click(&format!("wayland.window.{}", parent_id.0));
    assert!(
        !pressed(&harness, &parent.surface),
        "the parent hears no press"
    );
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&dialog.surface),
        "a press on the parent gives the dialog the keyboard"
    );

    harness.key(KEY_A);
    assert!(
        harness
            .client
            .received
            .seen
            .contains(&Seen::Key(dialog.surface.clone(), KEY_A)),
        "keys go to the dialog"
    );

    hint.unset_modal();
    harness.settle();
    harness.click(&format!("wayland.window.{}", parent_id.0));
    assert!(
        pressed(&harness, &parent.surface),
        "once the dialog is no longer modal the parent hears presses again"
    );
}
