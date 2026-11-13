use super::*;

const ARTWORK: &str = "#pan_zoom.card.4";

#[test]
fn reopening_a_block_keeps_the_view_the_person_moved() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Pan and Zoom");
    host.wait(ARTWORK);
    host.gone("#editor.loading");
    host.run(&["settle"]);
    let fitted = host.place(ARTWORK);
    let stage = host.place("#pan_zoom.stage");
    let middle = format!(
        "{:.0},{:.0}",
        stage.0 + stage.2 * 0.5,
        stage.1 + stage.3 * 0.5
    );

    host.run(&["keydown", "ctrl"]);
    host.run(&["wheel", &middle, "-2"]);
    host.run(&["keyup", "ctrl"]);
    host.run(&["settle"]);
    let zoomed = host.place(ARTWORK);
    assert!(
        zoomed.2 > fitted.2 * 1.05,
        "ctrl and the wheel zoom in, from {fitted:?} to {zoomed:?}"
    );

    host.click("#dock.tab.2.close");
    host.gone(ARTWORK);
    host.click("TreeItem \"Pan and Zoom\"");
    host.wait(ARTWORK);
    host.gone("#editor.loading");
    host.run(&["settle"]);
    let reopened = host.place(ARTWORK);
    assert!(
        (reopened.2 - zoomed.2).abs() < 1.0 && (reopened.0 - zoomed.0).abs() < 1.0,
        "the reopened block keeps the zoom the person chose, {reopened:?} rather than {zoomed:?}"
    );
    host.close();
}
