use super::*;

const GRIP: &str = "#dock.window.grip.block.2";
const PANE: &str = "Pane \"Files\"";

fn super_drag(host: &mut HostTest, from: (f32, f32), by: (f32, f32)) {
    let at = |step: f32| format!("{},{}", from.0 + by.0 * step, from.1 + by.1 * step);
    host.run(&["move", &at(0.0)]);
    host.run(&["keydown", "Super"]);
    host.run(&["down", &at(0.0)]);
    host.run(&["move", &at(0.5)]);
    host.run(&["move", &at(1.0)]);
    host.run(&["up"]);
    host.run(&["keyup", "Super"]);
    host.run(&["pause", "1000"]);
    host.run(&["settle"]);
}

#[test]
fn a_desktop_window_moves_by_super_drag_and_keeps_its_bar_on_the_screen() {
    let mut host = HostTest::start(Options {
        desktop: true,
        ..Options::default()
    });
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait(PANE);
    host.run(&["settle"]);

    for time in ["first", "second"] {
        let pane = host.place(PANE);
        let inside = (pane.0 + pane.2 / 2.0, pane.1 + pane.3 / 2.0);
        super_drag(&mut host, inside, (-40.0, -20.0));
        let moved = host.place(PANE);
        assert_eq!(
            (moved.0, moved.1),
            (pane.0 - 40.0, pane.1 - 20.0),
            "a {time} Super+drag on the window's content moves the window"
        );
    }

    let pane = host.place(PANE);
    let low = (pane.0 + pane.2 / 2.0, pane.1 + pane.3 - 20.0);
    super_drag(&mut host, low, (0.0, -low.1));
    let kept = host.place(GRIP);
    assert!(
        kept.1 >= 0.0,
        "a window carried by its content past the top springs back down until its grip shows: \
         {kept:?}"
    );
    host.snapshot("a_desktop_window_moves_by_super_drag_and_keeps_its_bar_on_the_screen");

    let from = (kept.0 + kept.2 / 2.0, kept.1 + kept.3 / 2.0);
    host.run(&["down", &format!("{},{}", from.0, from.1)]);
    host.run(&["move", &format!("{},{}", from.0 + 500.0, from.1)]);
    host.run(&["move", "1099,0"]);
    host.run(&["up"]);
    host.run(&["pause", "1000"]);
    host.run(&["settle"]);
    let kept = host.place(GRIP);
    assert!(
        kept.0 + kept.2 <= 1100.0,
        "a window let go off the right edge springs back until its grip shows: {kept:?}"
    );
    host.close();
}
