use super::*;
use crate::peer::forks::{Sharing, evicted, sharing_first};

/// A pass shares at most `SHARED_HEADS` heads, blocks the account's block holds none for
/// first, then those shared longest ago, so none waits behind the others for ever; and the
/// block keeps at most `KEPT_HEADS`, dropping those shared longest ago but never one just put.
#[test]
fn the_account_block_shares_a_bounded_set_of_heads() {
    let block = |index: u128| Uuid::from_u128(index);
    let mut candidates: Vec<Sharing> = (0..100)
        .map(|index| Sharing {
            block: block(index),
            held: Some(1_000 - index as i64),
        })
        .collect();
    candidates.push(Sharing {
        block: block(500),
        held: None,
    });
    let chosen = sharing_first(candidates, crate::peer::forks::SHARED_HEADS);
    assert_eq!(chosen.len(), 64, "a pass shares at most 64");
    assert_eq!(
        chosen[0].block,
        block(500),
        "a block holding no head goes first"
    );
    assert_eq!(
        chosen[1],
        Sharing {
            block: block(99),
            held: Some(901)
        },
        "then the one shared longest ago"
    );
    assert!(!chosen.iter().any(|sharing| sharing.block == block(0)));

    let held: Vec<(u32, i64)> = (0..6).map(|key| (key, 10 + key as i64)).collect();
    assert_eq!(
        evicted(&held, &[], 6),
        Vec::<u32>::new(),
        "within the bound nothing goes"
    );
    assert_eq!(
        evicted(&held, &[9], 6),
        vec![0],
        "the oldest goes for a new one"
    );
    assert_eq!(
        evicted(&held, &[0, 8, 9], 4),
        vec![1, 2, 3, 4],
        "one put again is never dropped, and the oldest of the rest go"
    );
    assert_eq!(evicted(&held, &[], 0), vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(crate::peer::forks::KEPT_HEADS, 512);
}
