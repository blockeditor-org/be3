use super::*;

#[test]
fn two_search_indexes_merge_by_parts_into_one_finding_both() {
    let base = applied(&SearchIndexContent::default(), 1, vec![put(1, 1, "apples")]);
    let ours = applied(&base, 2, vec![put(2, 2, "bananas")]);
    let theirs = applied(&base, 3, vec![put(3, 3, "cherries"), put(1, 4, "dates")]);
    let decoded = SearchIndexContent::decode(&ours.encode()).unwrap();
    assert_eq!(found(&decoded, "bananas"), [2]);

    let merged = match run(SearchIndexContent::merge_parts(
        base,
        ours,
        theirs,
        unread(),
    )) {
        Ok(MergeResult::Clean(merged)) => merged,
        other => panic!("the indexes did not merge cleanly: {:?}", other.err()),
    };
    assert_eq!(found(&merged, "bananas"), [2]);
    assert_eq!(found(&merged, "cherries"), [3]);
    assert_eq!(found(&merged, "dates"), [1]);
    assert!(
        found(&merged, "apples").is_empty(),
        "a document kept its words from before it was indexed again"
    );
}
