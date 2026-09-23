use super::*;

#[test]
fn map_entries_merge_key_by_key() {
    let base = Document::new(&Sheet {
        cells: [(1, "one".to_owned()), (2, "two".to_owned())]
            .into_iter()
            .collect(),
    });
    let mut ours = base.clone();
    ours.apply(&Sheet::CELLS.put(ObjectId::ROOT, &1, Some(&"uno".to_owned())).into());
    ours.apply(&Sheet::CELLS.put(ObjectId::ROOT, &3, Some(&"ours".to_owned())).into());
    let mut theirs = base.clone();
    theirs.apply(&Sheet::CELLS.put(ObjectId::ROOT, &2, None).into());
    theirs.apply(&Sheet::CELLS.put(ObjectId::ROOT, &3, Some(&"theirs".to_owned())).into());

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(cell(&merged, 1).as_deref(), Some("uno"));
    assert_eq!(cell(&merged, 2), None);
    assert_eq!(cell(&merged, 3).as_deref(), Some("ours"));
}
