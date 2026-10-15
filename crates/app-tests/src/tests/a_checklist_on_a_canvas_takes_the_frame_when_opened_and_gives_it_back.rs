use super::*;

#[test]
fn a_checklist_on_a_canvas_takes_the_frame_when_opened_and_gives_it_back() {
    let mut app = AppTest::start();
    add_root_block(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.block");
    app.click("Label value=\"Checklist\"");
    app.wait("Pane \"Checklist\"");
    app.record();

    app.double_click("Pane \"Checklist\"");
    app.gone("#infinite-canvas.dock");
    app.wait("#checklist.add");
    app.record();

    app.click("#checklist.draft");
    app.type_text("buy milk");
    app.click("#checklist.add");
    app.wait("CheckBox \"buy milk\"");
    app.record();

    app.key("Escape");
    app.wait("#infinite-canvas.dock");
    app.wait("CheckBox \"buy milk\"");
    app.snapshot("a_checklist_on_a_canvas_takes_the_frame_when_opened_and_gives_it_back");
    app.close();
}
