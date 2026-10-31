use super::*;

#[test]
fn a_slider_carries_its_value_and_range() {
    let shown = html(
        "div",
        with(Role::Slider, |node| {
            node.set_numeric_value(0.25);
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(1.0);
            node.set_value("25%");
            node.set_orientation(Orientation::Horizontal);
        }),
    );
    assert_eq!(
        shown,
        "<div role=\"slider\" aria-valuenow=\"0.25\" aria-valuemin=\"0\" aria-valuemax=\"1\" aria-valuetext=\"25%\" aria-orientation=\"horizontal\"></div>"
    );
}
