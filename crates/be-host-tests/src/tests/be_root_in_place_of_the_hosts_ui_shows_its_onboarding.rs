use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// The window's size in points.
const WIDTH: f32 = 1100.0;

/// Whether `color` is within a few levels of `expected` in each channel.
fn near(color: [u8; 3], expected: [u8; 3]) -> bool {
    color
        .iter()
        .zip(expected)
        .all(|(color, expected)| color.abs_diff(expected) <= 4)
}

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_its_onboarding() {
    // With no workspace the block stack waits for an account, so the root shows its accounts,
    // on be-host's block stack through be3:root, filling the window it is told of: something
    // shows past the 400 points the fixture's size once gave it.
    let mut host = HostTest::root(ROOT);
    host.run(&["settle"]);
    let (width, height, rgba) = host.shot();
    let scale = width as f32 / WIDTH;
    let pixel = |x: u32, y: u32| {
        let start = ((y * width + x) * 4) as usize;
        [rgba[start], rgba[start + 1], rgba[start + 2]]
    };
    let background = pixel(width - 2, height - 2);
    let past = (400.0 * scale) as u32;
    let drawn = (0..height).step_by(2).any(|y| {
        (past..width)
            .step_by(2)
            .any(|x| !near(pixel(x, y), background))
    });
    assert!(
        drawn,
        "the root's accounts show past 400 points, over the background {background:?} it fills the window with"
    );
    host.close();
}
