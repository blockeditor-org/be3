use super::*;

#[test]
fn a_selection_copies_a_wrapped_line_as_one_and_select_all_copies_unbuilt_history() {
    let mut harness = Harness::new(true);
    harness.show(&[prompt()]);
    harness.connect_host();
    let size = harness.size();
    let history: Vec<TerminalLine> = (0..2_000).map(log_line).collect();
    let mut rows = long_command(size.cols);
    rows.push(prompt());
    harness.hold(Screen::rows(rows).history(history));

    let line = harness.text_rect("echo the quick");
    harness.editor.drag(
        Pos2::new(line.left() + 1.0, line.top() + 3.0),
        Pos2::new(line.right() - 1.0, line.bottom() - 3.0),
    );
    harness.editor.settle();
    harness.editor.snapshot("a_selection_across_a_wrapped_line");
    let copied = harness.copy();
    assert_eq!(
        copied,
        [format!("ada@build-box:~$ {LONG}")],
        "a wrapped line copies without a newline where it wrapped"
    );
    assert!(
        harness
            .sent()
            .iter()
            .all(|message| !matches!(message, TerminalMessage::Key(_))),
        "Ctrl+Shift+C copies rather than typing"
    );

    harness.right_click(line.center());
    harness.click_text("Select All");
    harness.right_click(line.center());
    harness.click_text("Copy");
    let copied = harness.editor.take_copied_text();
    let lines: Vec<&str> = copied.last().expect("Copy copies").lines().collect();
    assert_eq!(
        lines.len(),
        2_002,
        "every history line, the wrapped line and the prompt"
    );
    assert_eq!(
        lines[0],
        log_line(0).text,
        "history rows that were never built copy too"
    );
    assert_eq!(lines[1_999], log_line(1_999).text);
    assert_eq!(lines[2_000], format!("ada@build-box:~$ {LONG}"));
}
