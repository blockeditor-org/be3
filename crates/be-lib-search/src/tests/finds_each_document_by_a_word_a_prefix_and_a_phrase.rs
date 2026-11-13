use super::*;

#[test]
fn finds_each_document_by_a_word_a_prefix_and_a_phrase() {
    let mut store = Store::default();
    let mut index = Index::default();
    for batch in 0..5 {
        let put = (batch * 60..batch * 60 + 60)
            .map(|n| put(n, 0, i64::from(n)))
            .collect();
        apply(
            &mut index,
            &mut store,
            id(batch),
            Batch {
                put,
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 5);
    for n in 0..300 {
        let word = run(index.search(&store, &Query::parse(&format!("uniq{n}v0x ")), 10, i64::MAX)).unwrap();
        assert_eq!(
            word.iter().map(|hit| hit.doc.clone()).collect::<Vec<_>>(),
            [doc(n)]
        );
        assert!(found(&index, &store, &format!("uniq{n}v")).contains(&doc(n)));
        assert!(found(&index, &store, &format!("MARK{n}")).contains(&doc(n)));
        assert_eq!(
            found(&index, &store, &format!("\"mark{n} sign{n}\"")),
            [doc(n)].into()
        );
        assert!(found(&index, &store, &format!("\"sign{n} mark{n}\"")).is_empty());
    }
    assert!(found(&index, &store, "uniq1v0x river").is_subset(&[doc(1)].into()));
    assert!(found(&index, &store, "nothingatall").is_empty());
}
