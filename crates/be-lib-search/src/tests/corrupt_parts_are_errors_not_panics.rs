use super::*;

/// The changes made to one part's bytes: cut short, and bytes flipped.
fn mutations(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(), bytes[..bytes.len() / 2].to_vec()];
    if let Some((_, rest)) = bytes.split_last() {
        out.push(rest.to_vec());
    }
    let step = (bytes.len() / 24).max(1);
    for at in (0..bytes.len()).step_by(step) {
        for value in [bytes[at] ^ 0xff, 0x80, 0] {
            let mut changed = bytes.to_vec();
            changed[at] = value;
            out.push(changed);
        }
    }
    out
}

/// Everything a reader or writer does with an index, whatever it answers.
fn exercise(index: &Index, other: &Index, store: &Store) {
    for query in ["river ", "uniq3v0x ", "uniq1", "\"mark5 sign5\"", "stone river "] {
        let _ = run(index.search(store, &Query::parse(query), 5));
    }
    let _ = run(index.entry(store, &doc(5)));
    let _ = run(index.entries_of(store, &doc(5).block));
    let mut changed = index.clone();
    let _ = run(changed.apply(store, id(90), Batch {
        put: vec![put(5, 2, 9)],
        removed: vec![(doc(6), commit(9))],
        removed_blocks: vec![(doc(7).block, commit(9))],
        watermark: None,
    }));
    let mut compacted = index.clone();
    let _ = run(compacted.compact(store, id(91)));
    let _ = run(index.merge(other, store, id(92)));
    let _ = run(other.merge(index, store, id(93)));
}

#[test]
fn corrupt_parts_are_errors_not_panics() {
    let mut store = Store::default();
    let mut index = Index::default();
    for batch in 0..4 {
        let mut put_list: Vec<Put> = (batch * 12..batch * 12 + 12).map(|n| put(n, 0, i64::from(batch))).collect();
        if batch > 0 {
            put_list.push(put(batch, 1, i64::from(batch)));
        }
        apply(&mut index, &mut store, id(batch), Batch { put: put_list, ..Batch::default() });
    }
    let other = index.clone();
    for name in index.parts() {
        let original = store.parts.parts[&name].clone();
        for changed in mutations(&original) {
            store.parts.parts.insert(name, changed);
            exercise(&index, &other, &store);
        }
        store.parts.parts.insert(name, original);
    }
    for changed in mutations(&index.encode()) {
        if let Ok(decoded) = Index::decode(&changed) {
            exercise(&decoded, &other, &store);
        }
    }
}
