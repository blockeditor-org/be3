use be_sdk_root_testing::{ORIGIN, SCREEN_SIZE};

use super::*;

/// The root built on be-sdk-root that embeds the triangle (crates/be-sdk-root-testing), staged as
/// an app.
const ROOT: &str = "be3.root-testing";

/// The window's width in points (`SIZE`).
const WIDTH: f32 = 1100.0;

#[test]
fn a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds() {
    let mut host = HostTest::root(ROOT);
    // The root owes frames until the triangle presented its screen.
    host.run(&["settle"]);
    let (width, height, rgba) = host.shot();
    let scale = width as f32 / WIDTH;
    let at = |x: f32, y: f32| {
        let (x, y) = ((x * scale) as u32, (y * scale) as u32);
        assert!(x < width && y < height, "({x}, {y}) is in the shot");
        let start = ((y * width + x) * 4) as usize;
        [rgba[start], rgba[start + 1], rgba[start + 2]]
    };
    let near = |left: [u8; 3], right: [u8; 3]| {
        left.iter()
            .zip(right)
            .all(|(left, right)| left.abs_diff(right) <= 4)
    };
    // The root shows the triangle's screen at `ORIGIN`, `SCREEN_SIZE` points; the triangle draws
    // in a square as tall as the screen in its middle (be-app-example-triangle's `paint`), its
    // corners at the top middle, bottom left and bottom right of the square's inner 80%.
    let (left, top) = ORIGIN;
    let (screen_width, screen_height) = (SCREEN_SIZE.0 as f32, SCREEN_SIZE.1 as f32);
    let side = screen_width.min(screen_height);
    let square = left + (screen_width - side) / 2.0;
    // Where the triangle's clip space falls in the window.
    let point = |x: f32, y: f32| (square + (x + 1.0) / 2.0 * side, top + (1.0 - y) / 2.0 * side);

    // The root's own surface is the window's plain background, and so is the screen where the
    // triangle drew nothing.
    let below = height as f32 / scale;
    let background = at(left / 2.0, (top + screen_height + below) / 2.0);
    let corner = at(left + 5.0, top + 5.0);
    assert!(
        near(corner, background),
        "the screen's corner, which the triangle leaves clear, shows the background {background:?}: {corner:?}"
    );
    // Below its top corner, which is red, the triangle is mostly red; beside its bottom left,
    // which is green, mostly green.
    let (x, y) = point(0.0, 0.7);
    let [red, green, blue] = at(x, y);
    assert!(
        red > green.saturating_add(60) && red > blue.saturating_add(60),
        "the triangle's top is red where the screen shows it: {:?}",
        [red, green, blue]
    );
    let (x, y) = point(-0.7, -0.55);
    let [red, green, blue] = at(x, y);
    assert!(
        green > red.saturating_add(40) && green > blue.saturating_add(20),
        "the triangle's bottom left is green: {:?}",
        [red, green, blue]
    );
    host.close();
}
