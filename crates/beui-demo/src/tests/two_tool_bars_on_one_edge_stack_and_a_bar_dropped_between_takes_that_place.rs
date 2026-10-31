use super::*;
use beui::WindowId;
use beui_host::native::testing::Target;

#[test]
fn two_tool_bars_on_one_edge_stack_and_a_bar_dropped_between_takes_that_place() {
    let mut test = demo(WIDE);
    open(&mut test, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    let style = test.rect_of("bars.style");
    assert!(
        tools.left() < style.left() && tools.right() <= style.left(),
        "the bar declared first floats nearer the left edge, the second beside it: {tools:?} {style:?}"
    );
    assert!(stage.contains_rect(tools) && stage.contains_rect(style));
    let kept = test.document().kept_clear(WindowId::MAIN);
    assert!(
        kept.iter()
            .any(|rect| rect.contains_rect(tools) && rect.contains_rect(style)),
        "what the stage keeps clear covers both bars: {kept:?}"
    );
    test.snapshot("two_tool_bars_float_over_the_left_of_their_stage");

    for grip in ["bars.grip", "bars.style.grip"] {
        test.click(grip);
        test.frame(Vec::new());
        test.click(Target::text("Dock along the bottom"));
        test.frame(Vec::new());
        test.settle();
    }
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    let style = test.rect_of("bars.style");
    assert!(
        tools.top() >= stage.bottom() && (style.top() - tools.top()).abs() < 1.0,
        "docked along the bottom, both bars share its one band: {stage:?} {tools:?} {style:?}"
    );
    assert!(
        tools.right() <= style.left() + 0.5,
        "side by side, the first bar first along the edge: {tools:?} {style:?}"
    );
    test.snapshot("two_tool_bars_docked_along_the_bottom_of_their_stage");

    let grip = test.rect_of("bars.style.grip").center();
    drop_bar(&mut test, grip, pos2(tools.left() + 8.0, tools.bottom() - 2.0));
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    let style = test.rect_of("bars.style");
    assert!(
        style.top() >= stage.bottom() && style.right() <= tools.left() + 0.5,
        "dropped over the start of the first bar, the second takes its place: {stage:?} {style:?} {tools:?}"
    );
    test.snapshot("a_tool_bar_dropped_before_another_along_the_edge_takes_its_place");
}
