use super::*;

use crate::diff::Diff;
use crate::seen::NewsQueue;

/// A hosted block's news (`Seen::member_news`) is kept by block and number: taking some leaves
/// the rest and other blocks' alone, and what the page is handed after one more is queued is
/// that one news, not the block's whole queue again.
#[test]
fn news_is_taken_by_number_and_an_update_carries_only_what_changed() {
    let (one, two) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let mut queue = NewsQueue::default();
    for said in 0..1000u32 {
        queue.push(one, said);
    }
    queue.push(two, 7);
    assert_eq!((queue.len(one), queue.len(two)), (1000, 1));

    let (news, last) = queue.of(one).expect("the block has news");
    assert_eq!(news.len(), 1000);
    assert_eq!(*news[999], 999);
    queue.took(one, last - 500);
    assert_eq!(queue.len(one), 500);
    assert_eq!(*queue.of(one).expect("half is left").0[0], 500);
    assert_eq!(queue.of(two).map(|(news, _)| news.len()), Some(1));

    let before = queue.clone();
    queue.push(one, 1000);
    let delta = NewsQueue::diff(&before, &queue).expect("the queue changed");
    let sent = postcard::to_allocvec(&delta).expect("the delta encodes");
    assert!(
        sent.len() < 64,
        "the delta carried {} bytes for one news",
        sent.len()
    );
    let mut applied = before;
    applied.apply(delta);
    assert_eq!(applied, queue);

    queue.clear(one);
    assert_eq!(queue.len(one), 0);
    assert!(queue.of(one).is_none());
}
