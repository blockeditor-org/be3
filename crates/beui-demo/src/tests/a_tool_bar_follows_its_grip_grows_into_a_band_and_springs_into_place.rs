use super::*;
use beui::{PointerButton, vec2};
use beui_host::native::testing::Target;

#[test]
fn a_tool_bar_follows_its_grip_grows_into_a_band_and_springs_into_place() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let grip = test.rect_of("bars.grip").center();

    test.down(Target::point(grip), PointerButton::Primary);
    test.hover(Target::point(grip + vec2(20.0, 0.0)));
    let held = pos2(stage.center().x - 60.0, stage.center().y + 40.0);
    test.hover(Target::point(held));
    for _ in 0..20 {
        test.frame(Vec::new());
    }
    let tools = test.rect_of("bars.tools");
    assert!(
        tools.contains(held) && stage.contains_rect(tools),
        "the bar is carried under the pointer: {tools:?} at {held:?}"
    );
    test.snapshot("a_tool_bar_carried_by_its_grip");

    test.hover(Target::point(pos2(stage.center().x, stage.top() + 6.0)));
    for _ in 0..4 {
        test.frame(Vec::new());
    }
    let band = test.rect_of("dock.bar.band.top");
    let tools = test.rect_of("bars.tools");
    assert!(
        band.height() < tools.height() - 2.0,
        "the band along the top is still growing open: {band:?} for {tools:?}"
    );
    test.snapshot("a_tool_bar_growing_into_the_band_along_the_top");

    test.hover(Target::point(held));
    test.pause(std::time::Duration::from_millis(200));
    let mut at = held;
    for _ in 0..4 {
        at += vec2(0.0, 40.0);
        test.hover(Target::point(at));
    }
    test.up(Target::point(at), PointerButton::Primary);
    test.frame(Vec::new());
    test.frame(Vec::new());
    let tools = test.rect_of("bars.tools");
    assert!(
        tools.bottom() < stage.bottom() - 4.0,
        "thrown down, the bar is still on its way: {tools:?} over {stage:?}"
    );
    test.snapshot("a_tool_bar_thrown_down_springing_into_the_band_along_the_bottom");

    test.settle();
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    assert!(
        tools.width() > tools.height() && tools.top() >= stage.bottom() - 0.5,
        "and settles docked along the bottom, below the stage: {tools:?} under {stage:?}"
    );
}
