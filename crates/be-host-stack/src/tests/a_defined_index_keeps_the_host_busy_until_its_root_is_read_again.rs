use uuid::Uuid;

use crate::search::Searching;

#[test]
fn a_defined_index_keeps_the_host_busy_until_its_root_is_read_again() {
    let root = Uuid::new_v4();
    let mut search = Searching::default();

    // A workspace that wants its index is busy with every queue empty, and not once it has one.
    assert!(search.busy(true));
    assert!(!search.busy(false));

    // A define in the outbox is not read yet; sent, it is, and the host stays busy until
    // the read ends.
    search.defined(root, 3);
    assert!(
        search.busy(false),
        "a define is on its way to the size shown"
    );
    assert!(
        search.confirm_due(1, true, 3).is_empty(),
        "still being sent"
    );
    assert!(search.busy(false));
    assert_eq!(search.confirm_due(0, false, 4), vec![root], "sent: read it");
    assert!(search.confirm_due(0, false, 4).is_empty(), "and only once");
    assert!(search.busy(false), "busy until the read ends");
    search.confirmed(root);
    assert!(!search.busy(false));

    // A define left in the outbox by a drain that ended since is not waited for.
    search.defined(root, 3);
    assert!(
        search.confirm_due(2, false, 3).is_empty(),
        "no drain has ended yet"
    );
    assert!(search.busy(false));
    assert!(search.confirm_due(2, false, 4).is_empty());
    assert!(!search.busy(false), "stuck: the old size is shown");
}
