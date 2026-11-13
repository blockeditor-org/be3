use super::*;

use std::time::Duration;

use crate::search::Searching;

#[test]
fn a_catch_up_settle_that_fails_to_write_is_settled_again_first() {
    let (index, gone) = (Uuid::new_v4(), Uuid::new_v4());
    let now = Duration::ZERO;
    let mut search = Searching::default();
    search.catch_page(index, 5, vec![gone], &[]);
    let settle = search.settled().remove(&index).expect("the page settles");

    // Put back first: a page caught up since settles after it, so what left the tree is still
    // removed, and the watermark moves to the later page.
    assert!(!search.settle_failed(index, settle, now));
    search.catch_page(index, 6, Vec::new(), &[]);
    let settle = search.settled().remove(&index).expect("both pages settle");
    assert_eq!(settle.sequence(), 6);
    assert_eq!(settle.left(), [gone]);

    // Counted with the index's batches: the third failure in a row sets it aside, and the host
    // says why. Searches are not complete until it is written.
    assert!(!search.write_failed(index, Vec::new(), Vec::new(), now));
    assert!(search.settle_failed(index, settle, now));
    assert!(search.settled().is_empty());
    assert!(!search.indexed());
    assert!(!search.busy(false));

    // The next connection puts it back.
    search.reconnected();
    let settle = search.settled().remove(&index).expect("it settles again");
    assert_eq!(settle.sequence(), 6);
    assert_eq!(settle.left(), [gone]);
}
