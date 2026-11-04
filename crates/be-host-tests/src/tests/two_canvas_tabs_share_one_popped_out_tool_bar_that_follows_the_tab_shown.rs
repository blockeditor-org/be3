use super::*;

const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const PEN_CHOSEN: &str = "Button \"Pen\" toggled=True";

#[test]
fn two_canvas_tabs_share_one_popped_out_tool_bar_that_follows_the_tab_shown() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.dock");
    start_adding_to_my_files(&mut host, "Canvas");
    host.type_text("Sketch");
    host.key("Enter");
    host.wait("TreeItem \"Sketch\"");
    host.wait("Tab \"Sketch\" selected");
    host.run(&["settle"]);
    let docked = host.place(CANVAS_BAR);

    host.click("#infinite-canvas.dock.grip");
    host.click("#grip.dock.bar.window");
    host.run(&["settle"]);
    let sketch = host.place(CANVAS_BAR);
    assert_ne!(sketch, docked, "the bar pops out into a window");
    host.click("#infinite-canvas.dock.tool.Pen");
    host.wait(PEN_CHOSEN);

    host.click("Tab \"Canvas\"");
    host.wait("Tab \"Canvas\" selected");
    host.run(&["settle"]);
    assert_eq!(
        host.count("#infinite-canvas.dock "),
        1,
        "the two canvases share one window"
    );
    assert_eq!(
        host.place(CANVAS_BAR),
        sketch,
        "the other canvas's bar popped out with it, into the same place"
    );
    assert!(
        !host.shown(PEN_CHOSEN),
        "the window shows the bar of the canvas shown now, with its own tool"
    );
    host.snapshot("two_canvas_tabs_share_one_popped_out_tool_bar_that_follows_the_tab_shown");

    host.click("Tab \"Sketch\"");
    host.wait("Tab \"Sketch\" selected");
    host.wait(PEN_CHOSEN);
    assert_eq!(host.place(CANVAS_BAR), sketch);

    host.click("#infinite-canvas.dock.grip");
    host.click("#grip.dock.bar.over.bottom");
    host.run(&["settle"]);
    assert_eq!(host.place(CANVAS_BAR), docked, "docking it back docks it");
    host.click("Tab \"Canvas\"");
    host.wait("Tab \"Canvas\" selected");
    host.run(&["settle"]);
    assert_eq!(
        host.place(CANVAS_BAR),
        docked,
        "and the other canvas's bar with it"
    );
    host.close();
}
