use be_root::{ROOT_COLOR, SIZE};

use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

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
fn be_root_in_place_of_the_hosts_ui_shows_its_onboarding() {
    // With no workspace the block stack waits for an account, so the root shows its accounts
    // over its own surface, on be-host's block stack through be3:root.
    let mut host = HostTest::root(ROOT);
    host.run(&["settle"]);
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let own = [own[0], own[1], own[2]];
    let (width, height) = (SIZE.0 as f32, SIZE.1 as f32);
    let corner = pixel(&mut host, width - 2.0, height - 2.0);
    let shown = (0..8).any(|row| {
        (0..8).any(|column| {
            let x = width * (column as f32 + 0.5) / 8.0;
            let y = height * (row as f32 + 0.5) / 8.0;
            !near(pixel(&mut host, x, y), corner)
        })
    });
    assert!(
        shown,
        "the root's accounts show something over its surface (its corner is {corner:?}, its own colour {own:?})"
    );
    host.close();
}
