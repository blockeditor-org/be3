use super::*;

const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const PEN_CHOSEN: &str = "Button \"Pen\" toggled=True";

#[test]
fn two_canvas_tabs_share_one_popped_out_tool_bar_that_follows_the_tab_shown() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.dock");
    start_adding_to_my_files(&mut app, "Canvas");
    app.type_text("Sketch");
    app.key("Enter");
    app.wait("TreeItem \"Sketch\"");
    app.wait("Tab \"Sketch\" selected");
    app.run(&["settle"]);
    let docked = app.place(CANVAS_BAR);

    app.click("#infinite-canvas.dock.grip");
    app.click("#grip.dock.bar.window");
    app.run(&["settle"]);
    let sketch = app.place(CANVAS_BAR);
    assert_ne!(sketch, docked, "the bar pops out into a window");
    app.click("#infinite-canvas.dock.tool.Pen");
    app.wait(PEN_CHOSEN);

    app.click("Tab \"Canvas\"");
    app.wait("Tab \"Canvas\" selected");
    app.run(&["settle"]);
    assert_eq!(
        app.count("#infinite-canvas.dock "),
        1,
        "the two canvases share one window"
    );
    assert_eq!(
        app.place(CANVAS_BAR),
        sketch,
        "the other canvas's bar popped out with it, into the same place"
    );
    assert!(
        !app.shown(PEN_CHOSEN),
        "the window shows the bar of the canvas shown now, with its own tool"
    );
    app.snapshot("two_canvas_tabs_share_one_popped_out_tool_bar_that_follows_the_tab_shown");

    app.click("Tab \"Sketch\"");
    app.wait("Tab \"Sketch\" selected");
    app.wait(PEN_CHOSEN);
    assert_eq!(app.place(CANVAS_BAR), sketch);

    app.click("#infinite-canvas.dock.grip");
    app.click("#grip.dock.bar.over.bottom");
    app.run(&["settle"]);
    assert_eq!(app.place(CANVAS_BAR), docked, "docking it back docks it");
    app.click("Tab \"Canvas\"");
    app.wait("Tab \"Canvas\" selected");
    app.run(&["settle"]);
    assert_eq!(
        app.place(CANVAS_BAR),
        docked,
        "and the other canvas's bar with it"
    );
    app.close();
}
