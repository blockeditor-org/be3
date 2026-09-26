use super::*;

#[test]
#[ignore = "clearing a row's last cell removes the row as it was seen, taking a cell someone set in it at the same time"]
fn a_cell_set_while_someone_clears_the_last_one_in_its_row_is_kept() {
    let (name, age) = (Uuid::new_v4(), Uuid::new_v4());
    let empty = DatabaseContent::default();
    let base = edited(
        &empty,
        [empty
            .root()
            .set_cell(0, name, Some(DatabaseValue::String("Ada".to_owned())))],
    );
    let clear = base.root().set_cell(0, name, None);
    let set = base
        .root()
        .set_cell(0, age, Some(DatabaseValue::Number(36.0)));

    let sequenced = edited(&base, [set, clear]);

    let rows = sequenced.root().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value(name), None);
    assert_eq!(rows[0].value(age), Some(&DatabaseValue::Number(36.0)));
}
