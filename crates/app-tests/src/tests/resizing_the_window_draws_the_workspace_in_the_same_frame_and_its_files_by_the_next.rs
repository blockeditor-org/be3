use super::*;

const WORKSPACE_UNDRAWN: &str =
    "the plugin be3.workspace-ui shows a view at a size it has yet to draw";
const UNDRAWN: &str = "shows a view at a size it has yet to draw";

#[test]
fn resizing_the_window_draws_the_workspace_in_the_same_frame_and_its_files_by_the_next() {
    let mut app = AppTest::start();
    app.wait("#file-tree.shared.row");
    app.run(&["move", "#file-tree.shared.row"]);
    app.run(&["settle"]);

    for size in ["1000x640", "1180x760"] {
        app.run(&["--no-settle", "resize", size]);
        let state = app.run(&["state"]);
        assert!(
            !state.contains(WORKSPACE_UNDRAWN),
            "the workspace is drawn at {size} in the frame the window takes it:\n{state}"
        );
        app.run(&["--no-settle", "pause", "0"]);
        let state = app.run(&["state"]);
        assert!(
            !state.contains(UNDRAWN),
            "a size the files only learn after they drew in that frame is drawn by the next one, at {size}:\n{state}"
        );
        app.run(&["settle"]);
    }
    app.close();
}
