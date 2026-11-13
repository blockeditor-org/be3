use super::*;

const PANE: &str = "Pane \"Checklist\"";

#[test]
fn a_checklist_on_a_canvas_works_in_place_and_zooms_with_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.wait(PANE);
    host.wait("#checklist.add");
    let pane = host.place(PANE);
    let button = host.place("#checklist.add");
    host.crop("#infinite-canvas.canvas");
    host.record();

    host.click("#infinite-canvas.pill.zoom-in");
    let zoomed = host.place(PANE);
    let zoomed_button = host.place("#checklist.add");
    let zoom = zoomed.2 / pane.2;
    assert!(
        zoom > 1.1,
        "zooming in grows the checklist, from {pane:?} to {zoomed:?}"
    );
    assert!(
        (zoomed_button.2 / button.2 - zoom).abs() < 0.05,
        "its content grows with it rather than laying out again, from {button:?} to {zoomed_button:?}"
    );
    host.click("#infinite-canvas.pill.zoom-out");

    host.run(&["move", "#checklist.draft"]);
    host.record();

    host.click("#checklist.draft");
    host.type_text("buy milk");
    host.click("#checklist.add");
    host.wait("CheckBox \"buy milk\"");
    host.wait("#infinite-canvas.dock");
    assert_eq!(
        host.place(PANE),
        pane,
        "the checklist stays where it is on the canvas"
    );
    host.snapshot("a_checklist_on_a_canvas_works_in_place_and_zooms_with_it");
    host.close();
}
