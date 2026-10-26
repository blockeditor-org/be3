use super::*;

#[test]
fn enabling_touch_emulation_in_the_inspector_does_not_paint_a_pointer() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::new(hello_column().document);

    harness.toggle_inspector();
    assert!(!harness.touch_emulation());

    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(vec![]);
    harness.click(harness.touch_toggle_center());
    assert!(harness.touch_emulation());

    let output = harness.frame(vec![]);
    assert!(!output.main().shapes().iter().any(|shape| matches!(
        shape,
        crate::painter::Shape::Rect {
            rect,
            corner_radius,
            ..
        } if rect.size() == Vec2::splat(20.0) && corner_radius.largest() == 10.0
    )));
}
