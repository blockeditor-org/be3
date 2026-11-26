use super::nested_restores::*;
use super::*;
use be_sdk_api::DockHold;

/// A child the root lets go before it takes the child's restore takes none of them, so they
/// count as taken: hosting the child again says so, and no asker waits for good.
#[test]
fn a_restore_still_queued_when_the_root_lets_its_child_go_is_counted_taken() {
    let mut session = docked_session();
    let child = ask_to_restore(&mut session);
    // What the root's bridge does as it stops hosting the child (`bridge_child_dock`).
    session.host().host_child_dock(child, DockHold::Released);
    let hosted = settled_hold(&mut session);
    assert!(
        !icons_only(&hosted),
        "the restore let go with the child is not taken"
    );
    assert_eq!(
        hosted.restored, 1,
        "the restore let go with the child counts as taken"
    );
}
