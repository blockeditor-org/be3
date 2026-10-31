use super::*;
use beui_host::native::testing::Target;

#[test]
fn a_tool_bar_moves_to_any_edge_of_its_pane_or_out_into_a_window() {
    let mut test = demo(WIDE);
    open(&mut test, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    assert!(
        tools.height() > tools.width() && stage.contains_rect(tools),
        "the bar floats over the left of the stage, laid out down it"
    );
    test.snapshot("a_tool_bar_floats_over_the_left_of_its_stage");

    test.click("bars.grip");
    test.frame(Vec::new());
    test.click(Target::text("Dock along the bottom"));
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let tools = test.rect_of("bars.tools");
    assert!(
        tools.width() > tools.height() && tools.top() >= stage.bottom(),
        "docked along the bottom, the bar takes a band below the stage"
    );
    test.snapshot("a_tool_bar_docked_along_the_bottom_of_its_stage");

    let grip = test.rect_of("bars.grip").center();
    drop_bar(&mut test, grip, pos2(stage.center().x, stage.top() + 4.0));
    test.frame(Vec::new());
    test.settle();
    let tools = test.rect_of("bars.tools");
    assert!(
        (tools.top() - stage.top()).abs() < 2.0,
        "dragged to the top edge, the bar docks along the top"
    );
    test.snapshot("a_tool_bar_dragged_to_the_top_of_its_stage");

    let grip = test.rect_of("bars.grip").center();
    drop_bar(&mut test, grip, pos2(60.0, WIDE.y - 80.0));
    test.frame(Vec::new());
    test.settle();
    let tools = test.rect_of("bars.tools");
    assert!(
        !stage.contains(tools.center()),
        "dragged away from its stage, the bar pops out into a window"
    );
    test.snapshot("a_tool_bar_popped_out_into_a_window");
}
