use super::*;

const SPLITTER: &str = "#dock.splitter.workspace";
const UNDRAWN: &str = "shows a view at a size it has yet to draw";

fn at(x: f32, y: f32) -> String {
    format!("{x:.0},{y:.0}")
}

#[test]
fn dragging_the_workspace_splitter_draws_the_files_at_their_new_width_in_the_same_frame() {
    let mut app = AppTest::start();
    app.wait(SPLITTER);
    app.wait("#file-tree.shared.row");
    app.run(&["settle"]);

    let (x, y, width, height) = app.place(SPLITTER);
    let middle = (x + width / 2.0, y + height / 2.0);
    app.run(&["down", &at(middle.0, middle.1)]);
    for step in 1..=3 {
        app.run(&[
            "--no-settle",
            "move",
            &at(middle.0 + 24.0 * step as f32, middle.1),
        ]);
        let state = app.run(&["state"]);
        assert!(
            !state.contains(UNDRAWN),
            "the files are drawn at the width the splitter gives them in the frame that moves it, step {step}:\n{state}"
        );
    }
    app.run(&["up"]);
    app.run(&["settle"]);
    let moved = app.place(SPLITTER);
    assert!(
        moved.0 > x + 48.0,
        "the drag moves the splitter, from {x} to {}",
        moved.0
    );
    app.close();
}
