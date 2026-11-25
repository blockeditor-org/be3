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

/// Types into the files' filter of the window by `grip`, which takes the keys: the filter in
/// that window is the one nearest below its grip, within its left edge.
fn type_into(host: &mut HostTest, grip: &str) {
    let grip = host.place(grip);
    let tree = host.tree();
    let filter = tree
        .lines()
        .filter(|line| line.contains("#file-tree.filter"))
        .filter_map(|line| {
            let (_, at) = line.split_once(" at ")?;
            let (at, _) = at.split_once(" size ")?;
            let (x, y) = at.split_once(',')?;
            Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))
        })
        // Below its grip and within its left edge, not another window's under it.
        .filter(|(x, y)| *y > grip.1 && *x >= grip.0)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("the window shows the files' filter below its grip");
    host.click(&format!("{},{}", filter.0 + 10.0, filter.1 + 10.0));
    host.type_text("zebra");
    host.wait("zebra");
}

/// Drags the window by `grip` with a finger by (40, 30) points: its grip moves by as much, and
/// once the finger lifts the window stays put as the mouse moves, a move begun by a touch ending
/// with it.
fn touch_drag(host: &mut HostTest, grip: &str) {
    let from = host.place(grip);
    let (x, y) = (from.0 + from.2 / 2.0, from.1 + from.3 / 2.0);
    host.run(&["touch", "down", &format!("0={x},{y}")]);
    for step in 1..=4 {
        let step = step as f32 / 4.0;
        host.run(&[
            "touch",
            "move",
            &format!("0={},{}", x + 40.0 * step, y + 30.0 * step),
        ]);
    }
    host.run(&["touch", "up"]);
    host.run(&["settle"]);
    let to = host.place(grip);
    assert!(
        (to.0 - from.0 - 40.0).abs() < 2.0 && (to.1 - from.1 - 30.0).abs() < 2.0,
        "the window follows the finger: its grip from {from:?} to {to:?}"
    );
    host.run(&["move", &format!("{},{}", x + 200.0, y + 150.0)]);
    host.run(&["settle"]);
    assert_eq!(
        host.place(grip),
        to,
        "the finger lifted, the window no longer follows the pointer"
    );
}

#[test]
fn be_root_in_place_of_the_hosts_ui_moves_a_window_the_desktop_opens_by_its_grip() {
    // A press on a desktop window's grip and a drag move the window with the pointer, under
    // be-host's UI and under the root alike: the desktop asks for the move (`begin-move`), and
    // be-host's desktop, or the root that floats its windows, follows the pointer until it is
    // released. Where it moved to, it takes a press and the keys typed then. Under the root a
    // finger moves it too, until it lifts.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    let grip = new_session(&mut host);
    drag(&mut host, &grip);
    type_into(&mut host, &grip);
    host.snapshot("the_host_moves_a_window_the_desktop_opens_by_its_grip");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    let grip = new_session(&mut host);
    drag(&mut host, &grip);
    touch_drag(&mut host, &grip);
    type_into(&mut host, &grip);
    host.snapshot("be_root_moves_a_window_the_desktop_opens_by_its_grip");
    host.close();
}
