use super::*;
use crate::profile::{MAX_RECENT, Recents};

fn blocks(recents: &Recents) -> Vec<Uuid> {
    recents.0.iter().map(|recent| recent.block).collect()
}

fn visited(recents: &Recents, block: Uuid) -> Recents {
    recents
        .visit(block, Uuid::nil())
        .unwrap_or_else(|| recents.clone())
}

#[test]
fn visiting_a_block_puts_it_first_in_the_recents_once() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let recents = visited(&Recents::default(), first);
    let recents = visited(&recents, second);
    assert_eq!(blocks(&recents), [second, first]);

    assert!(
        recents.visit(second, Uuid::nil()).is_none(),
        "visiting the first block again changes nothing"
    );

    let recents = visited(&recents, first);
    assert_eq!(blocks(&recents), [first, second], "a block is listed once");

    let mut recents = recents;
    for index in 0..MAX_RECENT as u128 + 5 {
        recents = visited(&recents, Uuid::from_u128(100 + index));
    }
    assert_eq!(blocks(&recents).len(), MAX_RECENT, "the list keeps its cap");

    let newest = blocks(&recents)[0];
    let recents = recents.forget(newest).expect("the block was listed");
    assert!(!blocks(&recents).contains(&newest));
    assert!(recents.forget(newest).is_none());
}
