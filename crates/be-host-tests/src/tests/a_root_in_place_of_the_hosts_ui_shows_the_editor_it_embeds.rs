use be_sdk_root_testing::{ORIGIN, ROOT_COLOR, SCREEN_SIZE, SIZE, TEXT_STRIP};

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
    let point = |x: f32, y: f32| {
        (
            square + (x + 1.0) / 2.0 * side,
            top + (1.0 - y) / 2.0 * side,
        )
    };

    // The root fills its own surface, `SIZE` points at the window's top left, with `ROOT_COLOR`,
    // which shows through the screen where the triangle drew nothing; past it the window shows
    // its plain background.
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let own = [own[0], own[1], own[2]];
    let beside = at(left / 2.0, top + 5.0);
    assert!(
        near(beside, own),
        "beside the screen, the root's own surface shows the colour it drew, {own:?}: {beside:?}"
    );
    let corner = at(left + 5.0, top + 5.0);
    assert!(
        near(corner, own),
        "the screen's corner, which the triangle leaves clear, shows the root's surface under it, \
         {own:?}: {corner:?}"
    );
    let past = at((SIZE.0 as f32 + width as f32 / scale) / 2.0, top + 5.0);
    assert!(
        !near(past, own),
        "past the root's surface the window shows its own background, not the root's colour: \
         {past:?}"
    );
    // Over its own surface the root draws a strip with beui, a line of text in the host's fonts
    // above the screen, clear around it: the text's glyphs stand out from the root's colour.
    let inked = (0..TEXT_STRIP.1)
        .flat_map(|y| (0..TEXT_STRIP.0 / 2).map(move |x| (x as f32, y as f32)))
        .filter(|&(x, y)| !near(at(x, y), own))
        .count();
    assert!(
        inked > 20,
        "the root's line of text shows above the screen, not {inked} points apart from its colour"
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
    // What the root painted of its own surface: its colour and its line of text, the painting
    // beui made of its document. The triangle's screen is not in it, since its app paints none.
    host.snapshot("a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds");
    host.close();
}
