use super::*;
use beui::PointerButton;

#[test]
fn a_chip_carried_out_of_its_bins_scroll_is_drawn_unclipped() {
    let mut test = alone(WIDE, Page::Dragging);
    reveal(&mut test, "demo.bin.2");
    let home = test.rect_of("demo.bin.0");
    let grab = home.center();
    let other = test.rect_of("demo.bin.3").center();
    // Half out of its bin, and then over the other bin: drawn whole, over both bins' edges.
    let between = pos2((grab.x + other.x) / 2.0, grab.y - home.height());
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(6.0, 0.0))]);
    test.frame(vec![Input::PointerMoved(between)]);
    test.settle();
    test.record();
    test.frame(vec![Input::PointerMoved(other)]);
    test.settle();
    test.record();
    assert_eq!(
        test.rect_of("demo.bin.0"),
        home,
        "carried, the chip keeps its place in its bin"
    );
    test.up(other, PointerButton::Primary);
    test.settle();
    test.record();
    test.snapshot("a_chip_carried_out_of_its_bins_scroll_is_drawn_unclipped");
}
