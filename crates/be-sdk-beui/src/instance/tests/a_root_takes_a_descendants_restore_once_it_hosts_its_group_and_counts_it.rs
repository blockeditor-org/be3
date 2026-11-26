use super::*;
use super::nested_restores::*;

#[test]
fn a_root_takes_a_descendants_restore_once_it_hosts_its_group_and_counts_it() {
    let mut session = docked_session();
    // The restore comes with the declaration that splices its group in, before the root hosts
    // the group.
    ask_to_restore(&mut session);
    let hosted = settled_hold(&mut session);
    assert!(
        icons_only(&hosted),
        "the root restores the nested dock's group once it hosts it"
    );
    assert_eq!(hosted.restored, 1, "the root's hold counts the restore it took");
}
