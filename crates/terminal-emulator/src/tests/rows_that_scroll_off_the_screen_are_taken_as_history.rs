use super::*;
use terminal_types::TerminalSize;

#[test]
fn rows_that_scroll_off_the_screen_are_taken_as_history() {
    let mut emulator = Emulator::new(TerminalSize { cols: 20, rows: 4 }).unwrap();
    let output: String = (1..=6).map(|line| format!("line {line}\r\n")).collect();
    emulator.feed(output.as_bytes());

    let (returned, scrolled) = emulator.take_scrolled();
    assert!(returned.is_empty());
    let scrolled: Vec<&str> = scrolled.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(scrolled, ["line 1", "line 2", "line 3"]);
    let shown = emulator.shown();
    let rows: Vec<&str> = shown.rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(rows, ["line 4", "line 5", "line 6", ""]);
    assert_eq!(shown.cursor.row, 3);
}
