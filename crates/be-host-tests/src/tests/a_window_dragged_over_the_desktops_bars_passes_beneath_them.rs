use super::*;

const GRIP: &str = "#dock.window.grip.block.2";
const CHIP: &str = "#desktop.windows.block.2";

#[test]
fn a_window_dragged_over_the_desktops_bars_passes_beneath_them() {
    let mut host = HostTest::start(Options {
        desktop: true,
        ..Options::default()
    });
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.wait(CHIP);
    host.run(&["settle"]);
    let band = host.place("#dock.bar.band.bottom");
    let chip = host.place(CHIP);
    let grip = host.place(GRIP);
    let pane = host.place("Pane \"Files\"");
    let x = grip.0 + grip.2 / 2.0;
    let from = grip.1 + grip.3 / 2.0;
    let to = from + band.1 + 14.0 - (pane.1 + pane.3);
    host.run(&["down", GRIP]);
    for step in 1..=4 {
        let y = from + (to - from) * step as f32 / 4.0;
        host.run(&["move", &format!("{x},{y}")]);
    }
    host.run(&["settle"]);
    let carried = host.place(GRIP);
    assert!(
        carried.1 > grip.1 + 20.0,
        "held, the window follows the pointer down toward the bars, its grip at {carried:?}"
    );
    let covered = host.place("Pane \"Files\"");
    assert!(
        covered.1 + covered.3 > chip.1 + 4.0,
        "the window reaches over the top of the Windows bar's chip: {covered:?} {chip:?}"
    );
    let over = format!("{},{}", chip.0 + chip.2 / 2.0, chip.1 + 2.0);
    let hit = host.run(&["hit", &over]);
    let innermost = hit
        .lines()
        .find(|line| line.trim_start().starts_with("interactive at"))
        .unwrap_or_default()
        .to_owned();
    assert!(
        innermost.contains("at 0,0 size 1100x720"),
        "a press on the Windows bar's chip reaches the shell's own view, drawn above the window carried over it, not the window: {hit}"
    );
    host.snapshot("a_window_dragged_over_the_desktops_bars_passes_beneath_them");
    host.run(&["up"]);
    host.run(&["settle"]);
    host.click(CHIP);
    host.close();
}
