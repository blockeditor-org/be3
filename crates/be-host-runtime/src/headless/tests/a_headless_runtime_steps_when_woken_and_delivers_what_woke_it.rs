use super::*;

use std::{cell::RefCell, rc::Rc};

/// With no display asking for frames, the runtime sleeps until something wakes it, and a step
/// delivers what did (plans/headless_apps.md).
#[test]
fn a_headless_runtime_steps_when_woken_and_delivers_what_woke_it() {
    let mut headless = Headless::start();
    let heard = Rc::new(RefCell::new(Vec::new()));
    let into = Rc::clone(&heard);
    let (_mailbox, post) = host::mailbox(move |letter: u32| into.borrow_mut().push(letter));
    headless.step();
    let sent = std::thread::spawn(move || post.send(7));
    let delivered = headless.run_until(Duration::from_secs(30), || {
        heard.borrow().first().copied()
    });
    sent.join().unwrap();
    assert_eq!(delivered, Some(7), "the letter from another thread woke a step that took it");
}
