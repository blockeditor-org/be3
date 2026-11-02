use super::*;

const UNDRAWN: &str = "shows a view at a size it has yet to draw";

#[test]
fn resizing_the_window_draws_the_workspace_and_its_files_at_the_new_size_in_the_same_frame() {
    let mut app = AppTest::start();
    app.wait("#file-tree.shared.row");
    app.run(&["move", "#file-tree.shared.row"]);
    app.run(&["settle"]);

    for size in ["1000x640", "1180x760"] {
        app.run(&["--no-settle", "resize", size]);
        let state = app.run(&["state"]);
        assert!(
            !state.contains(UNDRAWN),
            "the workspace and the files it places are drawn at {size} in the frame the window takes it:\n{state}"
        );
        app.run(&["settle"]);
    }
    app.close();
}
