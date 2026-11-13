use super::*;

/// Document `n` put by its name alone, as a device without its type's module puts it.
fn partial(n: u32, time: i64) -> Put {
    Put {
        text: Some(Text {
            name: format!("Note {n}"),
            body: String::new(),
        }),
        partial: true,
        ..put(n, 0, time)
    }
}

#[test]
fn a_full_put_replaces_a_partial_one_at_the_same_commit() {
    let mut store = Store::default();

    // Applied after the partial put, the full put of the same commit replaces it.
    let mut index = Index::default();
    for (origin, put) in [(1, partial(7, 2)), (2, put(7, 0, 2))] {
        apply(
            &mut index,
            &mut store,
            id(origin),
            Batch {
                put: vec![put],
                ..Batch::default()
            },
        );
    }
    assert_eq!(found(&index, &store, "uniq7v0x "), [doc(7)].into());
    let entry = run(index.entry(&store, &doc(7))).unwrap().unwrap();
    assert!(!entry.partial);

    // A partial put never replaces a full one, at the same commit or an older one.
    for (origin, time) in [(3, 2), (4, 1)] {
        apply(
            &mut index,
            &mut store,
            id(origin),
            Batch {
                put: vec![partial(7, time)],
                ..Batch::default()
            },
        );
    }
    assert_eq!(found(&index, &store, "uniq7v0x "), [doc(7)].into());

    // In one batch, the full put wins whichever comes first.
    let mut both = Index::default();
    apply(
        &mut both,
        &mut store,
        id(5),
        Batch {
            put: vec![put(8, 0, 2), partial(8, 2)],
            ..Batch::default()
        },
    );
    assert_eq!(found(&both, &store, "uniq8v0x "), [doc(8)].into());

    // Two devices' indexes, one with the partial entry and one with the full, merge to the
    // full one, either way round.
    let (mut named, mut read) = (Index::default(), Index::default());
    apply(
        &mut named,
        &mut store,
        id(6),
        Batch {
            put: vec![partial(9, 2)],
            ..Batch::default()
        },
    );
    apply(
        &mut read,
        &mut store,
        id(7),
        Batch {
            put: vec![put(9, 0, 2)],
            ..Batch::default()
        },
    );
    assert!(found(&named, &store, "uniq9v0x ").is_empty());
    for (origin, (a, b)) in [(8, (&named, &read)), (9, (&read, &named))] {
        let (merged, written) = run(a.merge(b, &store, id(origin))).unwrap();
        store.keep(&written);
        assert_eq!(found(&merged, &store, "uniq9v0x "), [doc(9)].into());
        let entry = run(merged.entry(&store, &doc(9))).unwrap().unwrap();
        assert!(!entry.partial);
    }
}
