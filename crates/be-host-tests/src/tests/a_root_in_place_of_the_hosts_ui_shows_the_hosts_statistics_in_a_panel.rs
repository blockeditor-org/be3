use be_sdk_root_testing::{ROOT_COLOR, STATISTICS_AREA, STATISTICS_ORIGIN};

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

#[test]
fn a_root_in_place_of_the_hosts_ui_shows_the_hosts_statistics_in_a_panel() {
    let mut host = HostTest::root(ROOT);
    host.run(&["settle"]);
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    let own = [own[0], own[1], own[2]];
    // The panel's bottom right corner, past its rows, over the root's own surface.
    let (x, y) = (
        STATISTICS_ORIGIN.0 + STATISTICS_AREA.0 as f32 - 5.0,
        STATISTICS_ORIGIN.1 + STATISTICS_AREA.1 as f32 - 5.0,
    );
    assert_eq!(pixel(&mut host, x, y), own);

    // F3 opens the panel, which watches be-host's statistics while it is open.
    host.key("F3");
    host.run(&["settle"]);
    assert_ne!(
        pixel(&mut host, x, y),
        own,
        "the panel shows over the root's own surface"
    );

    host.key("F3");
    host.run(&["settle"]);
    assert_eq!(
        pixel(&mut host, x, y),
        own,
        "F3 again closes it"
    );
    host.close();
}
