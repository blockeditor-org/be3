use super::*;

#[test]
fn the_scatter_plot_places_and_selects_a_point_for_each_row() {
    let mut fixture = editor(&[
        ("X", DatabaseFieldType::Number),
        ("Y", DatabaseFieldType::Number),
    ]);
    let (x, y) = (fixture.fields[0], fixture.fields[1]);
    fixture.set(0, x, DatabaseValue::Number(1.0));
    fixture.set(0, y, DatabaseValue::Number(1.0));
    fixture.set(1, x, DatabaseValue::Number(5.0));
    fixture.set(1, y, DatabaseValue::Number(9.0));
    fixture.edit_view(DatabaseView::set_scatter_x(Some(x)));
    fixture.edit_view(DatabaseView::set_scatter_y(Some(y)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Scatter));
    fixture.settle();

    let low = fixture.harness.rect_of("database-view.point.0");
    let high = fixture.harness.rect_of("database-view.point.1");
    assert!(low.left() < high.left(), "the smaller x sits to the left");
    assert!(low.top() > high.top(), "the smaller y sits lower");

    fixture.harness.click("database-view.point.1");
    fixture.settle();

    assert!(fixture.harness.shown("database-view.deselect"));
    assert!(
        fixture.harness.rect_of("database-view.point.1").width() > high.width(),
        "the selected point grows"
    );
    fixture
        .harness
        .snapshot("the_scatter_plot_places_and_selects_a_point_for_each_row");
}
