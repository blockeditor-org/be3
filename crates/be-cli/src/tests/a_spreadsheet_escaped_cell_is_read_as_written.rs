#[test]
fn a_spreadsheet_escaped_cell_is_read_as_written() {
    let records =
        crate::csv::parse("Title,Note\n'- beui: lists,'=SUM(A1)\nit's fine,'plain\n").unwrap();
    assert_eq!(
        records,
        [
            vec!["Title".to_owned(), "Note".to_owned()],
            vec!["- beui: lists".to_owned(), "=SUM(A1)".to_owned()],
            vec!["it's fine".to_owned(), "'plain".to_owned()],
        ]
    );
}
