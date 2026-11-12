use super::*;

/// The level is one of the desktop's overlays, which the host draws over the windows from the
/// shell's surface: only its card shows over the full screen window, never the desktop's
/// clear color around its corners or through its fade.
#[test]
fn the_volume_level_over_a_fullscreen_window_shows_only_its_card() {
    let mut host = HostTest::start_desktop();
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.run(&["settle"]);
    host.key("super+f");
    host.wait("#dock.away");
    host.run(&["settle"]);
    host.key("VolumeUp");
    host.wait("#desktop.osd.0");
    host.run(&["settle"]);
    host.snapshot("the_volume_level_over_a_fullscreen_window_shows_only_its_card");
    host.close();
}
