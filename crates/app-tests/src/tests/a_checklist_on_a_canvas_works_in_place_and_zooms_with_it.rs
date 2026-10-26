use super::*;

const PANE: &str = "Pane \"Checklist\"";

#[test]
fn a_checklist_on_a_canvas_works_in_place_and_zooms_with_it() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Checklist");
    app.wait(PANE);
    app.wait("#checklist.add");
    let pane = app.place(PANE);
    let button = app.place("#checklist.add");
    app.crop("#workspace.block");
    app.record();

    app.click("#infinite-canvas.pill.zoom-in");
    let zoomed = app.place(PANE);
    let zoomed_button = app.place("#checklist.add");
    let zoom = zoomed.2 / pane.2;
    assert!(
        zoom > 1.1,
        "zooming in grows the checklist, from {pane:?} to {zoomed:?}"
    );
    assert!(
        (zoomed_button.2 / button.2 - zoom).abs() < 0.05,
        "its content grows with it rather than laying out again, from {button:?} to {zoomed_button:?}"
    );
    app.click("#infinite-canvas.pill.zoom-out");

    app.run(&["move", "#checklist.draft"]);
    app.record();

    app.click("#checklist.draft");
    app.type_text("buy milk");
    app.click("#checklist.add");
    app.wait("CheckBox \"buy milk\"");
    app.wait("#infinite-canvas.dock");
    assert_eq!(
        app.place(PANE),
        pane,
        "the checklist stays where it is on the canvas"
    );
    app.snapshot("a_checklist_on_a_canvas_works_in_place_and_zooms_with_it");
    app.close();
}
