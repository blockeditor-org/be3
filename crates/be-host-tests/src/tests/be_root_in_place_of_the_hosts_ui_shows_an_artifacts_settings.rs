use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_an_artifacts_settings() {
    // be-host's own UI exports a canvas as a PNG image made again as the canvas changes, an
    // artifact; started again with the root in place of its UI, the artifact's Settings open
    // the dialog the workspace places, which runs the canvas's exporter's settings view, as
    // be-host runs it.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    // A shape, so the canvas has something to export.
    let stage = host.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    host.click("#infinite-canvas.dock.tool.Rectangle");
    host.run(&["drag", &at(0.3, 0.3), &at(0.45, 0.45)]);
    host.wait("GenericContainer #infinite-canvas.entity.");
    host.click("Button \"Close tab\"");
    host.gone("#infinite-canvas.canvas");
    host.run(&["click", "TreeItem \"Canvas\"", "right"]);
    host.click("#file-tree.export");
    host.wait("#file-tree.export.png");
    host.click("#file-tree.export.png");
    host.wait("#export.workspace");
    host.click("#export.workspace");
    host.gone("#export.workspace");
    host.wait("#workspace.artifact.settings");
    host.run(&["settle"]);
    // The artifact's bar shows in its tab and in the files' details: its Settings button in
    // the tab, the one on screen.
    let tree = host.tree();
    let line = tree
        .lines()
        .find(|line| line.contains("#workspace.artifact.settings") && !line.contains("offscreen"))
        .expect("the artifact's Settings button shows");
    let numbers: Vec<f32> = line
        .split(" at ")
        .nth(1)
        .expect("the button has a place")
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .filter_map(|number| number.parse().ok())
        .collect();
    let settings = format!(
        "{:.0},{:.0}",
        numbers[0] + numbers[2] / 2.0,
        numbers[1] + numbers[3] / 2.0
    );
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &settings]);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_an_artifacts_settings");
    // Its Background switch, then Apply: the workspace commits the child, and the host keeps
    // the settings and makes the image again with them.
    host.run(&["click", "688,379"]);
    host.run(&["settle"]);
    host.run(&["click", "674,419"]);
    host.run(&["settle"]);
    host.snapshot("be_root_applies_an_artifacts_settings");
    host.close();
}
