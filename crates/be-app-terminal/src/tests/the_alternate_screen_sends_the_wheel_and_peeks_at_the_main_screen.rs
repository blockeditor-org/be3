use super::*;

#[test]
fn the_alternate_screen_sends_the_wheel_and_peeks_at_the_main_screen() {
    let mut harness = Harness::new(true);
    harness.show(&[prompt()]);
    harness.connect_host();
    let size = harness.size();

    let mut editing = vec![
        TerminalLine::styled("# Notes", color(240, 200, 90)),
        plain(""),
        plain("Things to try in the terminal block:"),
        plain("- run vim and scroll with the wheel"),
        plain("- peek at the main screen"),
    ];
    while editing.len() + 1 < usize::from(size.rows) {
        editing.push(TerminalLine::styled("~", color(90, 110, 200)));
    }
    let mode = TerminalStyle {
        background: Some(TerminalColor {
            r: 90,
            g: 140,
            b: 255,
        }),
        bold: true,
        ..color(16, 18, 24)
    };
    let mut status = TerminalLine::styled(" NORMAL ", mode);
    status.push(" \"notes.md\" 5L, 120B", faint());
    editing.push(status);
    let history: Vec<TerminalLine> = (0..12).map(log_line).collect();
    harness.hold(
        Screen::rows(vec![command("vim notes.md")])
            .history(history)
            .alternate(editing),
    );

    assert!(harness.says("Things to try"));
    assert!(
        !harness.says("crate number"),
        "the program's screen alone is shown"
    );
    harness.editor.snapshot("the_alternate_screen");

    let screen = harness.editor.rect_of("terminal.screen");
    harness
        .editor
        .scroll(screen.center(), Vec2::new(0.0, -60.0));
    let wheel: Vec<_> = harness
        .sent()
        .into_iter()
        .filter_map(|message| match message {
            TerminalMessage::Wheel(wheel) => Some(wheel),
            _ => None,
        })
        .collect();
    assert!(
        wheel.iter().any(|wheel| wheel.lines < 0),
        "the wheel goes to the program: {wheel:?}"
    );

    harness.editor.click("terminal.peek");
    harness.editor.settle();
    assert!(
        harness.says("vim notes.md"),
        "the main screen shows while peeking"
    );
    assert!(harness.says("crate number 11"));
    harness.editor.snapshot("peeking_at_the_main_screen");
    harness.editor.click("terminal.screen");
    harness.editor.type_text("q");
    harness
        .editor
        .scroll(screen.center(), Vec2::new(0.0, -60.0));
    assert!(
        harness.sent().is_empty(),
        "the main screen is only looked at"
    );

    harness.editor.click("terminal.peek");
    harness.editor.settle();
    assert!(
        harness.says("Things to try"),
        "the button goes back to the program"
    );
}
