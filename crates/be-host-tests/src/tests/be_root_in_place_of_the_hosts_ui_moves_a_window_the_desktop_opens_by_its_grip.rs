use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// The grips of the desktop's dock windows the tree shows.
fn grips(host: &mut HostTest) -> Vec<String> {
    let tree = host.tree();
    let mut grips: Vec<String> = tree
        .split_whitespace()
        .filter(|word| word.starts_with("#dock.window.grip."))
        .map(str::to_owned)
        .collect();
    grips.sort();
    grips.dedup();
    grips
}

/// Opens a new session from the desktop's Apps panel, in a window of its own: the grip of that
/// window.
fn new_session(host: &mut HostTest) -> String {
    let before = grips(host);
    host.click("#desktop.apps");
    host.run(&["settle"]);
    host.click("#desktop.apps.sessions.new");
    host.run(&["settle"]);
    let after = grips(host);
    let new: Vec<&String> = after.iter().filter(|grip| !before.contains(grip)).collect();
    match new.as_slice() {
        [grip] => (*grip).clone(),
        _ => panic!("the new session opens in one new window: {before:?} then {after:?}"),
    }
}

/// Drags the window by `grip` by (-60, -40) points, up and to the left, staying on the screen: its grip moves by as much.
fn drag(host: &mut HostTest, grip: &str) {
    let from = host.place(grip);
    let (x, y) = (from.0 + from.2 / 2.0, from.1 + from.3 / 2.0);
    host.run(&["down", grip]);
    for step in 1..=4 {
        let step = step as f32 / 4.0;
        host.run(&["move", &format!("{},{}", x - 60.0 * step, y - 40.0 * step)]);
    }
    host.run(&["up"]);
    host.run(&["settle"]);
    let to = host.place(grip);
    assert!(
        (to.0 - from.0 + 60.0).abs() < 2.0 && (to.1 - from.1 + 40.0).abs() < 2.0,
        "the window follows the pointer by as much as it moved: its grip from {from:?} to {to:?}"
    );
}

#[test]
fn be_root_in_place_of_the_hosts_ui_moves_a_window_the_desktop_opens_by_its_grip() {
    // A press on a desktop window's grip and a drag move the window with the pointer, under
    // be-host's UI and under the root alike: the desktop asks for the move (`begin-move`), and
    // be-host's desktop, or the root that floats its windows, follows the pointer until it is
    // released.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    let grip = new_session(&mut host);
    drag(&mut host, &grip);
    host.snapshot("the_host_moves_a_window_the_desktop_opens_by_its_grip");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    let grip = new_session(&mut host);
    drag(&mut host, &grip);
    host.snapshot("be_root_moves_a_window_the_desktop_opens_by_its_grip");
    host.close();
}
