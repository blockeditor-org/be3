use crate::{Anchor, AnchorPlace, Terminal};

#[test]
fn rows_that_scroll_away_are_read_from_history() {
    let mut terminal = Terminal::new(10, 2, 100).unwrap();
    let anchor = Anchor::new(&terminal).unwrap();
    assert!(matches!(anchor.place(), AnchorPlace::Active(0)));

    terminal.write(b"one\r\ntwo\r\nthree\r\nfour");
    let colors = terminal.colors();

    let AnchorPlace::History(first) = anchor.place() else {
        panic!("the top row scrolled into history");
    };
    assert_eq!(first, 0);
    assert_eq!(terminal.scrollback_rows(), 2);
    let text = |y: usize| -> String {
        terminal
            .history_row(y, &colors)
            .unwrap()
            .cells
            .iter()
            .map(|cell| cell.text.clone())
            .collect()
    };
    assert_eq!(text(0), "one");
    assert_eq!(text(1), "two");
}
