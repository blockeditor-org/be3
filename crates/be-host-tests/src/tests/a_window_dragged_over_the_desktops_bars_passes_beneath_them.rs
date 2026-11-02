use super::*;

const GRIP: &str = "#dock.window.grip.block.2";
const CHIP: &str = "#desktop.windows.block.2";

#[test]
fn a_window_dragged_over_the_desktops_bars_passes_beneath_them() {
    let mut host = HostTest::start_desktop();
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.wait(CHIP);
    host.run(&["settle"]);
    let band = host.place("#dock.bar.band.bottom");
    let chip = host.place(CHIP);
    let grip = host.place(GRIP);
    let x = grip.0 + grip.2 / 2.0;
    let from = grip.1 + grip.3 / 2.0;
    let to = band.1 - 120.0;
    host.run(&["down", GRIP]);
    for step in 1..=4 {
        let y = from + (to - from) * step as f32 / 4.0;
        host.run(&["move", &format!("{x},{y}")]);
    }
    host.run(&["settle"]);
    let carried = host.place(GRIP);
    assert!(
        carried.1 > from + 100.0,
        "held, the window follows the pointer down toward the bars, its grip at {carried:?}"
    );
    let centre = format!("{},{}", chip.0 + chip.2 / 2.0, chip.1 + chip.3 / 2.0);
    let hit = host.run(&["hit", &centre]);
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
