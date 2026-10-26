use super::*;

const CAP: f32 = 280.0;

#[test]
fn a_tooltip_is_as_wide_as_its_label_up_to_a_cap() {
    let mut test = alone(WIDE, Page::Buttons);
    reveal(&mut test, "demo.tooltip.short");

    let short = hovered_tooltip(&mut test, "demo.tooltip.short");
    assert!(
        short.width() < CAP / 2.0,
        "a one-word tooltip shrinks to its label: {short:?}"
    );
    test.snapshot("a_tooltip_is_as_wide_as_its_label_up_to_a_cap");

    let long = hovered_tooltip(&mut test, "demo.tooltip.long");
    assert_eq!(
        long.width(),
        CAP,
        "a long tooltip stops at its cap and wraps its label"
    );
    test.snapshot("a_long_tooltip_wraps_at_its_cap");
}

fn hovered_tooltip(test: &mut DocumentTest, chip: &str) -> beui::Rect {
    let at = test.rect_of(chip).center();
    test.frame(vec![Input::PointerMoved(at)]);
    test.advance(std::time::Duration::from_secs(2));
    test.frame(Vec::new());
    test.settle();
    test.rect_of("tooltip")
}
