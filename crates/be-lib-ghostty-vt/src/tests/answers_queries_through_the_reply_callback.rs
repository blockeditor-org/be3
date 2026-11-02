use std::cell::RefCell;
use std::rc::Rc;

use super::*;

#[test]
fn answers_queries_through_the_reply_callback() {
    let mut terminal = Terminal::new(20, 3, 100).unwrap();
    let replies = Rc::new(RefCell::new(Vec::new()));
    let collected = replies.clone();
    terminal
        .on_reply(move |data| collected.borrow_mut().extend_from_slice(data))
        .unwrap();

    terminal.write(b"ab\r\ncd\x1b[6n");

    assert_eq!(replies.borrow().as_slice(), b"\x1b[2;3R");
}
