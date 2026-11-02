use super::*;

const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";

fn put(host: &mut HostTest, item: &str) -> (f32, f32, f32, f32) {
    host.click("#infinite-canvas.dock.grip");
    host.click(item);
    host.run(&["settle"]);
    host.place(CANVAS_BAR)
}

#[test]
fn a_canvas_tool_bar_goes_where_its_grip_menu_puts_it() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    let pane = host.place("Pane \"Canvas\"");

    let right = put(&mut host, "#grip.dock.bar.beside.right");
    assert!(
        right.3 > right.2 && (right.0 + right.2 - (pane.0 + pane.2)).abs() < 12.0,
        "docked along the right, the bar stands upright against it, at {right:?} in {pane:?}"
    );

    let left = put(&mut host, "#grip.dock.bar.over.left");
    assert!(
        left.3 > left.2 && left.0 > pane.0 + 2.0 && left.0 < pane.0 + 40.0,
        "floated over the left, it stands upright a little in from it, at {left:?} in {pane:?}"
    );

    let bottom = put(&mut host, "#grip.dock.bar.beside.bottom");
    assert!(
        bottom.2 > bottom.3 && (bottom.1 + bottom.3 - (pane.1 + pane.3)).abs() < 12.0,
        "docked along the bottom, it lies against it, at {bottom:?} in {pane:?}"
    );

    let band = host.place("#dock.bar.band.bottom");
    assert!(
        (band.0 - pane.0).abs() < 1.0 && (band.2 - pane.2).abs() < 1.0,
        "a bar docked along an edge sits in a band the whole length of it, {band:?}"
    );

    put(&mut host, "#grip.dock.bar.beside.top");
    host.snapshot("a_canvas_tool_bar_docked_along_the_top_sits_in_a_band");

    let popped = put(&mut host, "#grip.dock.bar.window");
    assert_ne!(popped, bottom, "popped out, it leaves the edge");
    host.snapshot("a_canvas_tool_bar_popped_out_from_its_grip_menu");

    let top = put(&mut host, "#grip.dock.bar.over.top");
    assert!(
        top.2 > top.3 && top.1 > pane.1 + 2.0 && top.1 < pane.1 + 40.0,
        "floated over the top from its window, it comes back to that edge, at {top:?}"
    );
    host.close();
}
