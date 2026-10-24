use std::cell::RefCell;
use std::rc::Rc;

use crate::Terminal;

#[test]
fn a_paste_is_bracketed_when_the_program_asks() {
    let mut terminal = Terminal::new(20, 3, 100).unwrap();
    let written = Rc::new(RefCell::new(Vec::new()));
    let collected = written.clone();
    terminal
        .on_reply(move |data| collected.borrow_mut().extend_from_slice(data))
        .unwrap();

    terminal.paste("one\ntwo").unwrap();
    assert_eq!(written.borrow().as_slice(), b"one\rtwo");

    written.borrow_mut().clear();
    terminal.write(b"\x1b[?2004h");
    terminal.paste("one\ntwo").unwrap();
    assert_eq!(written.borrow().as_slice(), b"\x1b[200~one\ntwo\x1b[201~");
}
