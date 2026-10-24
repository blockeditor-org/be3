use super::*;

#[test]
fn the_terminal_shows_history_and_a_wrapped_line_and_scrolls_back() {
    let mut harness = Harness::new(true);
    harness.show(&[prompt()]);
    harness.connect_host();
    let size = harness.size();

    let mut history: Vec<TerminalLine> = (0..40).map(log_line).collect();
    history.push(command("cat notes.txt"));
    history.push(plain(
        &"a history line longer than the terminal is wide wraps where the terminal did. ".repeat(2),
    ));
    history.push(command("ls"));
    history.push(listing());
    let mut rows = vec![command("claude --help")];
    rows.extend(long_command(size.cols));
    harness.hold(Screen::rows(rows).history(history));

    assert_eq!(harness.shown("terminal.host"), "\"ada on build-box\"");
    assert_eq!(harness.shown("terminal.status"), "\"\"");
    assert!(
        harness.says("crate number 39"),
        "the end of the history is in view"
    );
    assert!(harness.sent().is_empty(), "the screen follows the document");
    harness.editor.snapshot("a_running_terminal");

    let screen = harness.editor.rect_of("terminal.screen");
    for _ in 0..4 {
        harness
            .editor
            .scroll(screen.center(), Vec2::new(0.0, -120.0));
    }
    harness.editor.settle();
    assert!(harness.says("crate number 0 "), "the wheel scrolls back");
    assert!(
        harness.sent().is_empty(),
        "the main screen scrolls here, not on the host"
    );
    harness.editor.snapshot("a_terminal_scrolled_back");
}
