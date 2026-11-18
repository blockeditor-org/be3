use be_sdk_root_testing::{DIALOG_ORIGIN, ORIGIN, ROOT_COLOR, SCREEN_SIZE};

use super::*;

/// The root built on be-sdk-root that embeds the triangle (crates/be-sdk-root-testing), staged as
/// an app.
const ROOT: &str = "be3.root-testing";

/// The window's width in points.
const WIDTH: f32 = 1100.0;

/// The colour at `(x, y)`, in the window's points.
fn pixel(host: &mut HostTest, x: f32, y: f32) -> [u8; 3] {
    let (width, _height, rgba) = host.shot();
    let scale = width as f32 / WIDTH;
    let (x, y) = ((x * scale) as u32, (y * scale) as u32);
    let start = ((y * width + x) * 4) as usize;
    [rgba[start], rgba[start + 1], rgba[start + 2]]
}

/// Whether `color` is within a few levels of `expected` in each channel.
fn near(color: [u8; 3], expected: [u8; 3]) -> bool {
    color
        .iter()
        .zip(expected)
        .all(|(color, expected)| color.abs_diff(expected) <= 4)
}

#[test]
fn a_root_in_place_of_the_hosts_ui_opens_a_dialog_on_the_hosts_clipboard_and_input_method() {
    let mut host = HostTest::root(ROOT);
    host.run(&["settle"]);
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let own = [own[0], own[1], own[2]];
    // Left of the triangle's screen and of the dialog, where the root's own surface shows.
    let (left, top) = ORIGIN;
    let (beside_x, beside_y) = (left.min(DIALOG_ORIGIN.0) / 2.0, top + 5.0);
    assert!(near(pixel(&mut host, beside_x, beside_y), own));

    // A press focuses the triangle's screen, which tints its corner green.
    let inside = format!("#root+{},{}", left + 10.0, top + SCREEN_SIZE.1 as f32 / 2.0);
    host.run(&["click", &inside]);
    host.run(&["settle"]);

    // F2, which the root intercepts wherever the focus is, opens its dialog over a scrim.
    host.key("F2");
    host.run(&["settle"]);
    let dimmed = pixel(&mut host, beside_x, beside_y);
    assert!(
        dimmed.iter().zip(own).all(|(dimmed, own)| *dimmed < own) && dimmed != [0; 3],
        "the scrim dims the root's surface beside the dialog, {own:?}: {dimmed:?}"
    );
    host.snapshot(
        "a_root_in_place_of_the_hosts_ui_opens_a_dialog_on_the_hosts_clipboard_and_input_method",
    );

    // The dialog took the focus for its field: the platform's input method types into it, and
    // its clipboard is the window's.
    host.run(&["ime", "compose", "Mar"]);
    host.run(&["ime", "commit"]);
    host.run(&["clipboard", "ch"]);
    host.key("ctrl+v");
    host.run(&["settle"]);
    host.run(&["clipboard", ""]);
    host.key("ctrl+a");
    host.key("ctrl+c");
    host.run(&["settle"]);
    assert_eq!(
        host.run(&["clipboard"]).trim(),
        "March",
        "the field holds what the input method committed and what was pasted from the window's \
         clipboard, and copies it there"
    );

    // Escape, which the root intercepts while the dialog is open, closes it, and the focus goes
    // back to the triangle, whose key adds red to its corner's green.
    host.key("Escape");
    host.run(&["settle"]);
    assert!(
        near(pixel(&mut host, beside_x, beside_y), own),
        "the scrim went with the dialog"
    );
    host.key("a");
    host.run(&["settle"]);
    let [red, green, blue] = pixel(&mut host, left + 5.0, top + 5.0);
    assert!(
        red > 150 && green > 150 && blue < 150,
        "the triangle had the focus back, so the key reached it: {:?}",
        [red, green, blue]
    );

    // A press on the scrim beside the dialog closes it too.
    host.key("F2");
    host.run(&["settle"]);
    assert!(!near(pixel(&mut host, beside_x, beside_y), own));
    host.run(&["click", &format!("#root+{beside_x},{beside_y}")]);
    host.run(&["settle"]);
    assert!(
        near(pixel(&mut host, beside_x, beside_y), own),
        "a press on the scrim closed the dialog"
    );
    host.close();
}
