use super::*;

/// The root built on be-sdk-root that embeds the triangle (crates/be-sdk-root-testing), staged as
/// an app.
const ROOT: &str = "be3.root-testing";

/// The window's width in points (`SIZE`).
const WIDTH: f32 = 1100.0;

#[test]
fn a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds() {
    let mut host = HostTest::root(ROOT);
    // The root wants frames until the triangle presented its screen and wants no other.
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
    // The root's own surface is the window's plain background, and so is the screen where the
    // triangle drew nothing. The screen's top left is at (40, 20), 320 by 200 points.
    let background = at(20.0, 400.0);
    assert!(
        near(at(45.0, 25.0), background),
        "the screen's corner, which the triangle leaves clear, shows the background {background:?}: {:?}",
        at(45.0, 25.0)
    );
    // The triangle fills a square in the middle of the screen, from (100, 20) to (300, 220).
    // Below its top corner, which is red, it is mostly red; beside its bottom left, which is
    // green, mostly green.
    let [red, green, blue] = at(200.0, 50.0);
    assert!(
        red > green.saturating_add(60) && red > blue.saturating_add(60),
        "the triangle's top is red where the screen shows it: {:?}",
        [red, green, blue]
    );
    let [red, green, blue] = at(130.0, 175.0);
    assert!(
        green > red.saturating_add(40) && green > blue.saturating_add(20),
        "the triangle's bottom left is green: {:?}",
        [red, green, blue]
    );
    host.close();
}
