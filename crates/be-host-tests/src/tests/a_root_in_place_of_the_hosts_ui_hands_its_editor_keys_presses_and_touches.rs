use be_sdk_root_testing::{ORIGIN, SCREEN_SIZE};

use super::*;

/// The root built on be-sdk-root that embeds the triangle (crates/be-sdk-root-testing), staged as
/// an app.
const ROOT: &str = "be3.root-testing";

/// The window's width in points (`SIZE`).
const WIDTH: f32 = 1100.0;

/// The colour of the corner of the triangle's screen, which it leaves clear until it hears input
/// and then tints by what it heard (be-app-example-triangle's `Heard::tint`), and of the root's
/// own surface left of the screen, which shows through it while it is clear.
fn corner(host: &mut HostTest) -> ([u8; 3], [u8; 3]) {
    let (width, _height, rgba) = host.shot();
    let scale = width as f32 / WIDTH;
    let at = |x: f32, y: f32| {
        let (x, y) = ((x * scale) as u32, (y * scale) as u32);
        let start = ((y * width + x) * 4) as usize;
        [rgba[start], rgba[start + 1], rgba[start + 2]]
    };
    let (left, top) = ORIGIN;
    (at(left + 5.0, top + 5.0), at(left / 2.0, top + 5.0))
}

/// Whether `channel` is lit by the triangle's tint, which is 0.8 of full.
fn lit(channel: u8) -> bool {
    channel > 150
}

#[test]
fn a_root_in_place_of_the_hosts_ui_hands_its_editor_keys_presses_and_touches() {
    let mut host = HostTest::root(ROOT);
    host.run(&["settle"]);
    let (clear, background) = corner(&mut host);
    assert!(
        clear
            .iter()
            .zip(background)
            .all(|(clear, background)| clear.abs_diff(background) <= 4),
        "before any input the triangle leaves its screen's corner clear, showing the root's \
         surface {background:?}: {clear:?}"
    );
    // Inside the screen, away from the triangle: the root routes what lands there to it.
    let (left, top) = ORIGIN;
    let inside = format!("#root+{},{}", left + 10.0, top + SCREEN_SIZE.1 as f32 / 2.0);

    host.run(&["click", &inside]);
    host.run(&["settle"]);
    let ([red, green, blue], _) = corner(&mut host);
    assert!(
        lit(green) && !lit(red) && !lit(blue),
        "a press on the screen reaches the triangle, which tints it green: {:?}",
        [red, green, blue]
    );

    // The press focused the screen, so the key goes to the triangle.
    host.key("a");
    host.run(&["settle"]);
    let ([red, green, blue], _) = corner(&mut host);
    assert!(
        lit(red) && lit(green) && !lit(blue),
        "a key reaches the triangle the press focused, which adds red: {:?}",
        [red, green, blue]
    );

    host.run(&["tap", &inside]);
    host.run(&["settle"]);
    let ([red, green, blue], _) = corner(&mut host);
    assert!(
        lit(red) && lit(green) && lit(blue),
        "a touch reaches the triangle as a touch, not a press, and adds blue: {:?}",
        [red, green, blue]
    );
    host.close();
}
