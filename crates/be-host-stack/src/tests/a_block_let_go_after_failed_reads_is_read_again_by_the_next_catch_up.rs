use uuid::Uuid;

use crate::search::Searching;

#[test]
fn a_block_let_go_after_failed_reads_is_read_again_by_the_next_catch_up() {
    let block = Uuid::new_v4();
    let mut search = Searching::default();
    assert!(!search.read_failed(block), "held back the first time");
    assert!(!search.read_failed(block), "and the second");
    assert!(
        search.read_failed(block),
        "let go the third, so a catch-up that lists it ends"
    );
    assert!(search.take_queued().is_empty());

    search.retry_let_go();
    assert_eq!(
        search.take_queued(),
        vec![block],
        "the next catch-up reads it again, so its entry is not left stale"
    );
    assert!(
        !search.read_failed(block),
        "its failures count from none again"
    );

    search.read(block);
    search.retry_let_go();
    assert!(
        search.take_queued().is_empty(),
        "once read, it is not read again"
    );
}
