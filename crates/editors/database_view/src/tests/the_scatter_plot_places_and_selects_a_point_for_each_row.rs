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
    fixture
        .view
        .operate(DatabaseViewOperation::SetScatterXField { field_id: Some(x) });
    fixture
        .view
        .operate(DatabaseViewOperation::SetScatterYField { field_id: Some(y) });
    fixture.view.operate(DatabaseViewOperation::SetKind {
        kind: DatabaseViewKind::Scatter,
    });
    fixture.settle();

    let low = fixture.test.rect_of("database-view.point.0");
    let high = fixture.test.rect_of("database-view.point.1");
    assert!(low.left() < high.left(), "the smaller x sits to the left");
    assert!(low.top() > high.top(), "the smaller y sits lower");

    fixture.test.click("database-view.point.1");
    fixture.settle();

    assert!(fixture.test.shown("database-view.deselect"));
    assert!(
        fixture.test.rect_of("database-view.point.1").width() > high.width(),
        "the selected point grows"
    );
    fixture
        .test
        .snapshot("the_scatter_plot_places_and_selects_a_point_for_each_row");
}
