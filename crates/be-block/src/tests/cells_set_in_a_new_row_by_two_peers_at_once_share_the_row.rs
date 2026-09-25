use super::*;

#[test]
#[ignore = "set_cell addresses a row by index and inserts a fresh row when it is missing, so two peers filling the same new row make two rows"]
fn cells_set_in_a_new_row_by_two_peers_at_once_share_the_row() {
    let (name, age) = (Uuid::new_v4(), Uuid::new_v4());
    let base = DatabaseContent::default();
    let ours = base
        .root()
        .set_cell(0, name, Some(DatabaseValue::String("Ada".to_owned())));
    let theirs = base
        .root()
        .set_cell(0, age, Some(DatabaseValue::Number(36.0)));

    let sequenced = edited(&base, [ours, theirs]);

    let rows = sequenced.root().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].value(name),
        Some(&DatabaseValue::String("Ada".to_owned()))
    );
    assert_eq!(rows[0].value(age), Some(&DatabaseValue::Number(36.0)));
}
