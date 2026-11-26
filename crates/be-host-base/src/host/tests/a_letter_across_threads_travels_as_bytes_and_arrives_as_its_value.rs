use super::*;

/// A worker's letter goes as its bytes (`Post::across`), as it must to a page whose memory the
/// worker does not share, and the mailbox's handler hears the value it was made from.
#[test]
fn a_letter_across_threads_travels_as_bytes_and_arrives_as_its_value() {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let hearing = Rc::clone(&heard);
    let (_mailbox, post) = mailbox(move |letter: (String, u32)| hearing.borrow_mut().push(letter));
    let across = post.across();
    thread::spawn(move || across.send(("from a worker".to_owned(), 7)))
        .join()
        .unwrap();
    deliver();
    assert_eq!(*heard.borrow(), vec![("from a worker".to_owned(), 7)]);
}
