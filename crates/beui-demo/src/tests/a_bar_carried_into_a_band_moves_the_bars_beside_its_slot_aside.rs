use super::*;
use beui::{PointerButton, vec2};
use beui_host::native::testing::Target;

#[test]
fn a_bar_carried_into_a_band_moves_the_bars_beside_its_slot_aside() {
    let mut test = demo(WIDE);
    open(&mut test, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    for grip in ["bars.grip", "bars.style.grip"] {
        test.click(grip);
        test.frame(Vec::new());
        test.click(Target::text("Dock along the bottom"));
        test.frame(Vec::new());
        test.settle();
    }
    let tools = test.rect_of("bars.tools");
    let style = test.rect_of("bars.style");
    assert!(
        tools.right() <= style.left() + 0.5,
        "docked along the bottom, the first bar first: {tools:?} {style:?}"
    );

    let grip = test.rect_of("bars.style.grip").center();
    let over = pos2(tools.left() + 8.0, tools.center().y);
    test.down(Target::point(grip), PointerButton::Primary);
    test.hover(Target::point(grip + vec2(0.0, -20.0)));
    test.hover(Target::point(over));
    test.frame(Vec::new());
    test.frame(Vec::new());
    let moving = test.rect_of("bars.tools");
    assert!(
        moving.left() > tools.left() && moving.left() < tools.left() + style.width(),
        "carried over the start of the first bar, the slot opens before it and the bar springs aside: {moving:?}, was {tools:?}"
    );
    test.snapshot("a_bar_carried_into_a_band_moving_the_bar_beside_its_slot_aside");

    test.settle();
    let aside = test.rect_of("bars.tools");
    assert!(
        aside.left() >= moving.left() && aside.left() > tools.left() + style.width() / 3.0,
        "and comes to rest past the slot: {aside:?}, was {tools:?}"
    );
    test.up(Target::point(over), PointerButton::Primary);
    test.settle();
    let style = test.rect_of("bars.style");
    assert!(
        style.right() <= test.rect_of("bars.tools").left() + 0.5,
        "let go, it takes that slot"
    );
}
