use super::*;
use block_editor_beui::beui::Vec2;

const ROWS: usize = 200;

#[test]
fn a_row_scrolled_into_a_reused_line_opens_the_row_it_shows() {
    let mut fixture = text_editor();
    let name = fixture.fields[0];
    for row in 0..ROWS {
        fixture.set(row, name, DatabaseValue::String(format!("Row {row}")));
    }
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();
    let rows = fixture.database();
    let line = |index: usize| {
        format!(
            "database-view.list.row.{}",
            rows.rows.get_index(index).unwrap().id.as_uuid()
        )
    };

    let target = ROWS - 3;
    for _ in 0..100 {
        if fixture.harness.shown(&line(target)) {
            break;
        }
        let top = (0..ROWS)
            .find(|index| fixture.harness.shown(&line(*index)))
            .unwrap();
        fixture.harness.scroll(line(top), Vec2::new(0.0, 300.0));
        fixture.settle();
    }

    assert!(
        fixture
            .harness
            .label(&line(target))
            .contains(&format!("Row {target}"))
    );
    assert!(!fixture.harness.shown(&line(0)));
    fixture.harness.click(line(target));
    fixture.settle();
    let id = rows.rows.get_index(target).unwrap().id;
    assert_eq!(
        fixture.harness.label("database-view.detail.id"),
        rows.short_id(id)
    );
}
