use super::*;

#[test]
fn returning_to_the_inspector_tab_repaints_the_whole_panel() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);
    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.click(harness.bar_option_center(0));
    harness.frame(Vec::new());

    let inspector = harness.bar_option_center(1);
    let mut damage = Vec::new();
    harness.frame(vec![Event::PointerMoved(inspector)]);
    for pressed in [true, false] {
        let output = harness.frame(vec![Event::PointerButton {
            pos: inspector,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
        damage.extend_from_slice(output.damage.rects());
    }
    damage.extend_from_slice(harness.frame(Vec::new()).damage.rects());

    let panel = harness
        .inspector()
        .panel_rect()
        .expect("the inspector panel is shown")
        .scaled(crate::inspector::scale(&harness.context));
    assert!(panel.top() >= harness.bar_option_rect(1).bottom());
    assert_eq!(panel.right(), VIEWPORT.x);
    let mut y = panel.top() + 0.5;
    while y < panel.bottom() {
        let mut x = panel.left() + 0.5;
        while x < panel.right() {
            assert!(
                damage.iter().any(|rect| rect.contains(pos2(x, y))),
                "the panel at {x},{y} was not repainted, damage was {damage:?}"
            );
            x += 8.0;
        }
        y += 8.0;
    }
}
