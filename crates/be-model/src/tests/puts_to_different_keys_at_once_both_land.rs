use super::*;

#[test]
fn puts_to_different_keys_at_once_both_land() {
    let base = sheet(&[(1, "one")]);
    let ours: Edit = Sheet::CELLS
        .put(ObjectId::ROOT, &2, Some(&"two".to_owned()))
        .into();
    let theirs: Edit = Sheet::CELLS.put(ObjectId::ROOT, &1, None).into();

    let mut ours_first = base.clone();
    ours_first.apply(&ours);
    ours_first.apply(&theirs);
    let mut theirs_first = base;
    theirs_first.apply(&theirs);
    theirs_first.apply(&ours);

    assert_eq!(ours_first, theirs_first);
    assert_eq!(cell(&ours_first, 1), None);
    assert_eq!(cell(&ours_first, 2).as_deref(), Some("two"));
}
