use super::*;

#[test]
fn a_query_reads_few_blocks_while_skipping_a_common_term() {
    let mut store = Store::default();
    let mut index = Index::default();
    let put = (0..1200)
        .map(|n| crate::Put {
            doc: doc(n),
            commit: commit(1),
            source: None,
            text: Some(Text {
                name: format!("entry{n}"),
                body: if n == 777 { "common rare".into() } else { format!("common filler{n}") },
            }),
        })
        .collect();
    apply(&mut index, &mut store, id(1), Batch { put, ..Batch::default() });
    store.reads.borrow_mut().clear();
    let hits = run(index.search(&store, &Query::parse("common rare "), 10)).unwrap();
    assert_eq!(hits.iter().map(|hit| hit.doc.clone()).collect::<Vec<_>>(), [doc(777)]);
    let dictionary = store.count(|kind| *kind == PartKind::Dictionary);
    let postings = store.count(|kind| *kind == PartKind::Postings);
    // "common"'s 1200 postings are ten blocks; the query reads its skip data
    // and the one block holding "rare"'s document.
    assert!(dictionary <= 4, "{dictionary} dictionary blocks read");
    assert_eq!(postings, 3, "skip data and a block of common, and rare");
    assert_eq!(store.count(|kind| *kind == PartKind::Positions), 0);

    // Alone, a common term's top ten skip most of its list by block bounds.
    store.reads.borrow_mut().clear();
    let hits = run(index.search(&store, &Query::parse("common "), 10)).unwrap();
    assert_eq!(hits.len(), 10);
    let postings = store.count(|kind| *kind == PartKind::Postings);
    assert!(postings <= 4, "{postings} postings reads");
}
