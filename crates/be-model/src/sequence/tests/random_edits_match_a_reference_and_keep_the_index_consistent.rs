use super::*;

#[test]
fn random_edits_match_a_reference_and_keep_the_index_consistent() {
    let start = "the quick brown fox";
    let mut sequence = loaded(start);
    let mut reference = Reference::new(start);
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let mut undo: Vec<SeqOp<u8>> = Vec::new();

    for _ in 0..1200 {
        let client = 1 + random.below(3) as u64;
        let len = sequence.len();
        let letters = |random: &mut Random| -> Vec<u8> {
            (0..1 + random.below(4))
                .map(|_| b'a' + random.below(26) as u8)
                .collect()
        };
        let op = match random.below(9) {
            0..=2 => {
                let items = letters(&mut random);
                sequence.insert(client, random.below(len + 1), items)
            }
            3 => {
                let from = random.below(len + 1);
                sequence.delete(from..(from + random.below(6)).min(len))
            }
            4 => {
                let from = random.below(len + 1);
                let items = letters(&mut random);
                sequence.replace(client, from..(from + random.below(4)).min(len), items)
            }
            5 if len > 0 => {
                let from = random.below(len);
                let to = (from + 1 + random.below(5)).min(len);
                sequence.move_range(from..to, random.below(len + 1))
            }
            6 | 7 => undo.pop(),
            _ => {
                let anchor = reference.order[random.below(reference.order.len())].0;
                Some(SeqOp::Insert {
                    after: Some(anchor),
                    client,
                    start: sequence.next_offset(client),
                    items: letters(&mut random),
                })
            }
        };
        let Some(op) = op else {
            continue;
        };
        if let Some((back, _)) = sequence.inverse(&op) {
            undo.push(back);
        }
        let before = sequence.items();
        let splices = sequence.apply(&op);

        assert_eq!(splices.is_some(), reference.apply(&op), "{op:?}");
        assert_eq!(order(&sequence), reference.order, "{op:?}");
        check(&sequence);
        let after = sequence.items();
        if let Some(splices) = splices {
            let removed: usize = splices.iter().map(|splice| splice.removed).sum();
            let inserted: usize = splices.iter().map(|splice| splice.inserted).sum();
            assert_eq!(before.len() - removed + inserted, after.len());
            if let [splice] = splices.as_slice() {
                let mut replayed = before.clone();
                replayed.splice(
                    splice.at..splice.at + splice.removed,
                    after[splice.at..splice.at + splice.inserted]
                        .iter()
                        .copied(),
                );
                assert_eq!(replayed, after);
            }
        }
    }

    assert!(sequence.fragment_count() > 2 * CHUNK);
    assert!(sequence.chunks.len() > 1);
}
