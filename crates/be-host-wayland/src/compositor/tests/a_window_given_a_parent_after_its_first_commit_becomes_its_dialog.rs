use super::*;

#[test]
fn a_window_given_a_parent_after_its_first_commit_becomes_its_dialog() {
    let mut harness = Harness::new();
    let (parent, parent_id) = harness.open();
    let dialog = harness.client.toplevel();
    harness.settle();
    let toplevel = dialog.toplevel.as_ref().expect("the dialog is a toplevel");
    toplevel.set_parent(parent.toplevel.as_ref());
    let serial = harness
        .client
        .received
        .configured
        .take()
        .expect("the dialog was configured");
    dialog.xdg_surface.ack_configure(serial);
    harness.client.attach_unsent(&dialog, 300, 200);
    harness.settle();
    let parent_of = |harness: &mut Harness| {
        harness
            .compositor()
            .windows()
            .list()
            .get_untracked()
            .into_iter()
            .find(|info| info.id != parent_id)
            .map(|info| info.parent)
    };

    assert_eq!(parent_of(&mut harness), Some(Some(parent_id)));

    toplevel.set_parent(None);
    harness.settle();

    assert_eq!(
        parent_of(&mut harness),
        Some(None),
        "a window whose parent is cleared is a window of its own again"
    );
}
