use super::*;

const DOCKED: &str = "#dock.window.grip.block.3";
const FLOATING: &str = "#dock.window.grip.block.2";
const SHELL: &str = "at 0,0 size 1100x720";

fn new_session(host: &mut HostTest, chip: &str) {
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait(chip);
    host.run(&["settle"]);
}

fn innermost(host: &mut HostTest, x: f32, y: f32) -> String {
    let hit = host.run(&["hit", &format!("{x},{y}")]);
    hit.lines()
        .find(|line| line.trim_start().starts_with("interactive at"))
        .unwrap_or_default()
        .trim()
        .to_owned()
}

#[test]
fn a_window_over_a_tab_docked_on_the_desktop_draws_above_it() {
    let mut host = HostTest::start_desktop();
    new_session(&mut host, "#desktop.windows.block.2");
    new_session(&mut host, "#desktop.windows.block.3");
    let grip = host.place(DOCKED);
    let tab = format!("{},{}", grip.0 + grip.2 + 40.0, grip.1 + grip.3 / 2.0);
    host.run(&["drag", &tab, "12,360", "12"]);
    host.gone(DOCKED);
    host.run(&["settle"]);

    let docked = innermost(&mut host, 12.0, 600.0);
    assert!(
        !docked.is_empty() && !docked.contains(SHELL),
        "the session docked along the left edge takes presses where no window covers it: {docked}"
    );
    let floating = host.place(FLOATING);
    let y = floating.1 + 300.0;
    let mut x = floating.0 + 10.0;
    while x < 540.0 {
        let reached = innermost(&mut host, x, y);
        assert!(
            reached != docked && !reached.contains(SHELL),
            "a press at {x},{y} on the window over the docked session, its splitter included, reaches the window: {reached}"
        );
        x += 6.0;
    }
    host.snapshot("a_window_over_a_tab_docked_on_the_desktop_draws_above_it");
    host.close();
}
