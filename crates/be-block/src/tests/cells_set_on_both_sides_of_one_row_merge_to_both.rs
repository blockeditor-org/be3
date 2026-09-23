use super::*;
use uuid::Uuid;

#[test]
fn cells_set_on_both_sides_of_one_row_merge_to_both() {
    let (name, done) = (Uuid::new_v4(), Uuid::new_v4());
    let mut base = DatabaseContent::default();
    let edit = base
        .root()
        .set_cell(0, name, Some(DatabaseValue::String("Write".to_owned())));
    base.apply(&edit);
    let ours = edited(
        &base,
        [base
            .root()
            .set_cell(0, name, Some(DatabaseValue::String("Rewrite".to_owned())))],
    );
    let theirs = edited(
        &base,
        [base
            .root()
            .set_cell(0, done, Some(DatabaseValue::Boolean(true)))],
    );

    let merged = match Merge::merge3(&base, &ours, &theirs) {
        be_commit::MergeResult::Clean(merged) => merged,
        other => panic!("expected a clean merge, got {other:?}"),
    };

    let rows = merged.root().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].value(name),
        Some(&DatabaseValue::String("Rewrite".to_owned()))
    );
    assert_eq!(rows[0].value(done), Some(&DatabaseValue::Boolean(true)));
}
