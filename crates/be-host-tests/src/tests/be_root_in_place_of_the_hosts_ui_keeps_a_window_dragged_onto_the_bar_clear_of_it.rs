use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_keeps_a_window_dragged_onto_the_bar_clear_of_it() {
    // Under the root, a desktop window dragged by its grip down onto the desktop's bottom bar
    // passes over it while held, and once released is kept with its grip clear of the bar, on
    // the work area the desktop's view keeps (`frame-report.work-area`), as be-host's desktop
    // keeps one (`a_window_dragged_over_the_desktops_bars_passes_beneath_them`).
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#desktop.apps");
    host.run(&["settle"]);
    host.click("#desktop.apps.sessions.new");
    host.run(&["settle"]);
    let grips: Vec<String> = host
        .tree()
        .split_whitespace()
        .filter(|word| word.starts_with("#dock.window.grip."))
        .map(str::to_owned)
        .collect();
    let grip = grips
        .last()
        .cloned()
        .expect("the new session opens in a window with a grip");
    let band = host.place("#dock.bar.band.bottom");
    let from = host.place(&grip);
    let (x, y) = (from.0 + from.2 / 2.0, from.1 + from.3 / 2.0);
    let to = band.1 + band.3 / 2.0;
    host.run(&["down", &grip]);
    for step in 1..=4 {
        let step = step as f32 / 4.0;
        host.run(&["move", &format!("{x},{}", y + (to - y) * step)]);
    }
    host.run(&["settle"]);
    let held = host.place(&grip);
    assert!(
        held.1 + held.3 > band.1,
        "held, the window follows the pointer onto the bar: its grip at {held:?}, the bar at {band:?}"
    );
    host.run(&["up"]);
    host.run(&["settle"]);
    let kept = host.place(&grip);
    assert!(
        kept.1 + kept.3 <= band.1 + 0.5,
        "released, its grip is kept clear of the bar: at {kept:?}, the bar at {band:?}"
    );
    host.snapshot("be_root_keeps_a_window_dragged_onto_the_bar_clear_of_it");
    host.close();
}
