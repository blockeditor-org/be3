use super::*;

const UNDRAWN: &str = "shows a view at a size it has yet to draw";

#[test]
fn resizing_the_window_draws_the_workspace_and_its_files_at_the_new_size_in_the_same_frame() {
    let mut host = HostTest::start(Options::default());
    host.wait("#file-tree.shared.row");
    host.run(&["move", "#file-tree.shared.row"]);
    host.run(&["settle"]);

    for size in ["1000x640", "1180x760"] {
        host.run(&["--no-settle", "resize", size]);
        let state = host.run(&["state"]);
        assert!(
            !state.contains(UNDRAWN),
            "the workspace and the files it places are drawn at {size} in the frame the window takes it:\n{state}"
        );
        host.run(&["settle"]);
    }
    host.close();
}
