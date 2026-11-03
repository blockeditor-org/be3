use super::*;

#[test]
fn a_window_whose_parent_is_not_shown_is_a_window_of_its_own() {
    let mut harness = Harness::new();
    let (dialog, dialog_id) = harness.open();
    let hidden = harness.client.toplevel();
    harness.settle();
    let serial = harness
        .client
        .received
        .configured
        .take()
        .expect("the parent was configured");
    dialog
        .toplevel
        .as_ref()
        .expect("the dialog is a toplevel")
        .set_parent(hidden.toplevel.as_ref());
    harness.settle();
    let listed = |harness: &mut Harness| {
        harness
            .compositor()
            .windows()
            .list()
            .get_untracked()
            .into_iter()
            .map(|info| (info.id, info.parent))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        listed(&mut harness),
        vec![(dialog_id, None)],
        "a parent that never showed is not listed, so the dialog stands alone"
    );

    hidden.xdg_surface.ack_configure(serial);
    harness.client.attach_unsent(&hidden, 300, 200);
    harness.settle();
    let parent_id = listed(&mut harness)
        .into_iter()
        .map(|(id, _)| id)
        .find(|id| *id != dialog_id)
        .expect("the parent is listed once it shows");

    assert!(
        listed(&mut harness).contains(&(dialog_id, Some(parent_id))),
        "once the parent shows, the dialog is its own"
    );
}
