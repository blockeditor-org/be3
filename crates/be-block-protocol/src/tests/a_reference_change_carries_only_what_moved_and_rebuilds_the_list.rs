use super::*;

fn ids(seeds: &[u128]) -> Vec<Uuid> {
    seeds.iter().map(|seed| Uuid::from_u128(*seed)).collect()
}

#[test]
fn a_reference_change_carries_only_what_moved_and_rebuilds_the_list() {
    let before = ids(&(1..=1000).collect::<Vec<_>>());
    let cases: Vec<Vec<Uuid>> = vec![
        before.clone(),
        Vec::new(),
        {
            let mut added = before.clone();
            added.insert(400, Uuid::from_u128(5000));
            added
        },
        {
            let mut removed = before.clone();
            removed.remove(10);
            removed
        },
        {
            let mut moved = before.clone();
            let block = moved.remove(900);
            moved.insert(3, block);
            moved
        },
        before.iter().rev().copied().collect(),
        ids(&[7, 3, 9000, 2]),
    ];
    for after in cases {
        let change = ReferenceChange::between(&before, &after);
        assert_eq!(change.apply(&before), Some(after.clone()));
        let moved = change.removed.len() + change.added.len();
        match after.len() {
            1000 if after == before => assert!(change.is_empty()),
            1001 | 999 => assert_eq!(moved, 1, "one reference added or removed"),
            1000 if after[0] == before[0] => {
                assert_eq!(moved, 2, "a moved reference is removed and added")
            }
            _ => {}
        }
    }
    assert!(ReferenceChange::between(&[], &[]).is_empty());

    // Removing a reference the list lacks changes nothing, and one past the end goes last.
    let tolerant = ReferenceChange {
        removed: ids(&[42, 1]),
        added: vec![PlacedReference {
            index: 99,
            block: Uuid::from_u128(1),
        }],
    };
    assert_eq!(tolerant.apply(&ids(&[1, 2])), Some(ids(&[2, 1])));
    // Any change `between` would not make is refused: adding one already there, naming one
    // twice, or indices out of order.
    let placed = |index: u32, seed: u128| PlacedReference {
        index,
        block: Uuid::from_u128(seed),
    };
    for added in [
        vec![placed(0, 2)],
        vec![placed(0, 7), placed(1, 7)],
        vec![placed(1, 7), placed(0, 8)],
    ] {
        let change = ReferenceChange {
            removed: Vec::new(),
            added,
        };
        assert_eq!(change.apply(&ids(&[1, 2])), None, "{change:?}");
    }
}
