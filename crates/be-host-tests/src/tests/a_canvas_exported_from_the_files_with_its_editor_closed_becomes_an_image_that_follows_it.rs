use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

/// The files pane's Export as runs the canvas's exporters with its editor closed: the host
/// runs the Export dialog in the canvas's app, Copy puts the PNG on the clipboard, and Add to
/// workspace makes an image block that is made again as the canvas changes.
#[test]
fn a_canvas_exported_from_the_files_with_its_editor_closed_becomes_an_image_that_follows_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let draw = |host: &mut HostTest, from: (f32, f32), to: (f32, f32)| {
        let stage = host.place("#infinite-canvas.canvas");
        let at = |(x, y): (f32, f32)| {
            format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y)
        };
        host.click("#infinite-canvas.dock.tool.Rectangle");
        host.run(&["drag", &at(from), &at(to)]);
    };
    draw(&mut host, (0.3, 0.3), (0.45, 0.45));
    host.wait(SHAPE);
    host.click("Button \"Close tab\"");
    host.gone("#infinite-canvas.canvas");

    let export = |host: &mut HostTest| {
        host.run(&["click", "TreeItem \"Canvas\"", "right"]);
        host.click("#file-tree.export");
        host.wait("#file-tree.export.svg");
        host.click("#file-tree.export.png");
        host.wait("#export.workspace");
    };
    export(&mut host);
    host.wait("Export as PNG image");
    host.snapshot("a_canvas_exported_from_the_files_shows_its_export_dialog");
    host.click("#export.copy");
    host.gone("#export.copy");
    host.gone("#infinite-canvas.canvas");

    export(&mut host);
    host.click("#export.workspace");
    host.gone("#export.workspace");
    host.wait("Tab \"Canvas.png\"");
    host.wait("#workspace.artifact.source");

    host.click("TreeItem \"Canvas\"");
    host.wait("#infinite-canvas.canvas");
    let drawn = host.count(SHAPE);
    draw(&mut host, (0.55, 0.3), (0.7, 0.45));
    assert_eq!(host.count(SHAPE), drawn + 1, "a second shape is drawn");
    host.click("Tab \"Canvas.png\"");
    host.wait("#workspace.artifact.source");
    host.pause(600);
    host.snapshot("a_canvas_exported_to_the_workspace_follows_it");
    host.close();
}
