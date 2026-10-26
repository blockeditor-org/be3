use crate::{ActiveScreen, Terminal};

#[test]
fn rows_carry_their_soft_wrap() {
    let mut terminal = Terminal::new(5, 3, 100).unwrap();
    terminal.write("abcdefg\r\n世界".as_bytes());
    let colors = terminal.colors();

    let first = terminal.active_row(0, &colors).unwrap();
    let second = terminal.active_row(1, &colors).unwrap();
    let third = terminal.active_row(2, &colors).unwrap();

    assert!(first.wrapped);
    assert!(!second.wrapped);
    assert_eq!(first.cells.len(), 5);
    assert_eq!(first.cells[4].text, "e");
    assert_eq!(second.cells[1].text, "g");
    assert_eq!(third.cells[0].text, "世");
    assert!(third.cells[1].spacer);
    assert_eq!(third.cells[2].text, "界");
    assert_eq!(terminal.active_screen(), ActiveScreen::Primary);

    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.active_screen(), ActiveScreen::Alternate);
}
