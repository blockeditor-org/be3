use super::*;
use super::nested_restores::*;
use be_sdk_api::{DockDeclaredChild, DockUpdate};

/// A descendant's restore waits for the root to host its group; once the child stops declaring
/// the group, it never will, so the root drops the restore and counts it taken.
#[test]
fn a_descendants_restore_waiting_for_a_group_the_child_stops_declaring_is_dropped_and_counted() {
    let mut session = docked_session();
    let child = ask_to_restore(&mut session);
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(unspliced()),
            epoch: 2,
        })]);
    let hosted = settled_hold(&mut session);
    assert!(!icons_only(&hosted), "the dropped restore changes nothing");
    assert_eq!(hosted.restored, 1, "the root counts the dropped restore taken");
}
