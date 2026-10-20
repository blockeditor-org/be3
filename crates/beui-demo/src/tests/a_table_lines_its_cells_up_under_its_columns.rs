use super::*;

#[test]
fn a_table_lines_its_cells_up_under_its_columns() {
    let mut test = alone(WIDE, Page::Layout);
    test.frame(Vec::new());
    reveal(&mut test, "table.jupiter");
    for column in ["year", "moons"] {
        let short = test.rect_of(&format!("table.mercury.{column}"));
        let long = test.rect_of(&format!("table.jupiter.{column}"));
        assert_eq!(short.left(), long.left(), "{column}");
        assert_eq!(short.width(), long.width(), "{column}");
    }
    assert!(
        test.rect_of("table.jupiter").height() > test.rect_of("table.mercury").height(),
        "a caption under a value makes its row taller"
    );
    test.snapshot("a_table");
}
