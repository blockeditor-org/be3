use uuid::Uuid;

use crate::search::Searching;

#[test]
fn a_catch_up_moves_its_watermark_only_past_blocks_that_were_put() {
    let index = Uuid::new_v4();
    let (first, second, third) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (gone, later_gone) = (Uuid::new_v4(), Uuid::new_v4());

    // Three pages, the second with a block whose commit could not be fetched: the watermark
    // stops at the first page, and the third is not counted though it was put.
    let mut search = Searching::default();
    search.catch_page(index, 10, vec![gone], &[first]);
    search.catch_page(index, 20, vec![], &[second]);
    search.catch_page(index, 30, vec![later_gone], &[third]);
    search.put(first);
    search.put(third);
    let settled = search.settled();
    let settle = &settled[&index];
    assert_eq!(settle.sequence(), 10, "only the first page was indexed");
    assert_eq!(settle.left(), &[gone], "and only its removals are masked");
    assert!(search.settled().is_empty(), "nothing is left to settle");

    // A put by a live edit, which no page lists, is not remembered for a page that lists the
    // block later and could not read it.
    let stale = Uuid::new_v4();
    search.put(stale);
    search.catch_page(index, 40, vec![], &[stale]);
    assert!(search.settled().is_empty(), "a page's block was not put for it");

    // With every block put, the watermark reaches the last page.
    search.catch_page(index, 10, vec![gone], &[first]);
    search.catch_page(index, 30, vec![later_gone], &[third]);
    search.put(first);
    search.put(third);
    let settled = search.settled();
    assert_eq!(settled[&index].sequence(), 30);
    assert_eq!(settled[&index].left(), &[gone, later_gone]);
}
