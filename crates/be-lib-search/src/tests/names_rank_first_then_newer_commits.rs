use super::*;

#[test]
fn names_rank_first_then_newer_commits() {
    let mut store = Store::default();
    let mut index = Index::default();
    let document = |n: u32, time: i64, name: &str, body: &str| Put {
        doc: doc(n),
        commit: commit(time),
        source: None,
        text: Some(Text {
            name: name.into(),
            body: body.into(),
        }),
    };
    let first = vec![
        document(1, 1, "Groceries", "buy apples and pears"),
        document(2, 2, "Apples", "a list of fruit"),
        document(3, 3, "Notes", "buy apples and pears"),
    ];
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: first,
            ..Batch::default()
        },
    );
    let second = vec![
        document(4, 4, "Plans", "buy apples and pears"),
        document(5, 0, "Old", "buy apples and pears"),
    ];
    apply(
        &mut index,
        &mut store,
        id(2),
        Batch {
            put: second,
            ..Batch::default()
        },
    );
    let hits = run(index.search(&store, &Query::parse("apples "), 10, i64::MAX)).unwrap();
    let order: Vec<DocId> = hits.iter().map(|hit| hit.doc.clone()).collect();
    assert_eq!(order, [doc(2), doc(4), doc(3), doc(1), doc(5)]);
    let top = run(index.search(&store, &Query::parse("apples "), 2, i64::MAX)).unwrap();
    assert_eq!(
        top.iter().map(|hit| hit.doc.clone()).collect::<Vec<_>>(),
        [doc(2), doc(4)]
    );
    // A prefix reaches the name too.
    assert_eq!(found(&index, &store, "groc"), [doc(1)].into());
}
