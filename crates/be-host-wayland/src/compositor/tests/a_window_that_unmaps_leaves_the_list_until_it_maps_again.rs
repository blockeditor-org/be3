use super::*;

#[test]
fn a_window_that_unmaps_leaves_the_list_until_it_maps_again() {
    let mut harness = Harness::new();
    let (parent, parent_id) = harness.open();
    let (dialog, dialog_id) = harness.open();
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    harness.settle();
    let listed = |harness: &mut Harness| {
        harness
            .compositor()
            .windows()
            .list()
            .get_untracked()
            .into_iter()
            .find(|info| info.id == dialog_id)
    };
    assert_eq!(
        listed(&mut harness).map(|info| info.parent),
        Some(Some(parent_id))
    );
    harness.client.received.configured = None;

    dialog.surface.attach(None, 0, 0);
    dialog.surface.commit();
    harness.settle();

    assert!(
        listed(&mut harness).is_none(),
        "a window that attaches no buffer is no longer listed"
    );
    assert!(
        harness
            .output
            .as_ref()
            .unwrap()
            .test_id_rect(&format!("wayland.window.{}", dialog_id.0))
            .is_none(),
        "the unmapped window is no longer shown"
    );
    let serial = harness
        .client
        .received
        .configured
        .take()
        .expect("an unmapped window is configured again before it maps");

    toplevel.set_parent(None);
    dialog.xdg_surface.ack_configure(serial);
    harness.client.attach_unsent(&dialog, 300, 200);
    harness.settle();

    assert_eq!(
        listed(&mut harness).map(|info| info.parent),
        Some(None),
        "a window that maps again is listed afresh"
    );
}
