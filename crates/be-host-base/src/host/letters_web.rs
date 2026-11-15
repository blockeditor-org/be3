//! On the web the page takes no lock another thread takes (plans/web_page_never_waits.md), so
//! a letter waits in a list of the page's own. One the page posts to itself goes straight onto
//! it. A worker posts its letters in bytes (`Across`): they are copied out of its memory and
//! handed to the page by `postMessage` (`web/threads.js`, "be-letter-post"), and the page's
//! handler copies them in and puts them on the list (`be_letter_arrived`) and wakes the host.
//! A worker's letters reach the page in the order it posted them (each thread posts on one
//! channel); letters from different threads arrive in no set order between them, and a
//! worker's letter arrives a task after it was posted, not at once.
//!
//! Only the page makes mailboxes: it is the one thread whose frames deliver them.

use std::{cell::RefCell, thread, thread::ThreadId};

use wasm_bindgen::prelude::wasm_bindgen;

use super::{Encoded, Letter, wake};

thread_local! {
    static ARRIVED: RefCell<Vec<Letter>> = const { RefCell::new(Vec::new()) };
}

// unsafe(ffi)
#[link(wasm_import_module = "wasi")]
unsafe extern "C" {
    #[link_name = "be-letter-post"]
    fn be_letter_post(mailbox: f64, bytes: *const u8, length: usize);
}

pub(super) fn post(to: ThreadId, letter: Letter) {
    if to == thread::current().id() {
        arrived(letter);
        return;
    }
    // A worker's letter is bytes the page copies (`Post::across`): a value in this memory would
    // not reach a page whose memory the worker does not share.
    eprintln!("be-host: a worker posted a letter as a value; it posts across threads in bytes");
}

pub(super) fn post_bytes(to: ThreadId, mailbox: u64, bytes: Vec<u8>) {
    if to == thread::current().id() {
        arrived((mailbox, Box::new(Encoded(bytes))));
        return;
    }
    // `threads.js` copies the bytes out before it returns, so they are let go of here.
    // unsafe(ffi)
    unsafe { be_letter_post(mailbox as f64, bytes.as_ptr(), bytes.len()) };
}

fn arrived(letter: Letter) {
    ARRIVED.with_borrow_mut(|arrived| arrived.push(letter));
    wake();
}

/// A letter a worker posted in bytes (`post_bytes`), handed over by the page's handler for its
/// message.
#[wasm_bindgen]
pub fn be_letter_arrived(mailbox: f64, bytes: Vec<u8>) {
    arrived((mailbox as u64, Box::new(Encoded(bytes))));
}

/// Takes, in the order they arrived, the letters whose mailbox `at_this_desk` answers true for,
/// and drops those whose mailbox is gone (`None`).
pub(super) fn take(_here: ThreadId, at_this_desk: impl Fn(u64) -> Option<bool>) -> Vec<Letter> {
    let mut letters = Vec::new();
    let mut kept = Vec::new();
    let mut gone = Vec::new();
    for letter in ARRIVED.take() {
        match at_this_desk(letter.0) {
            Some(true) => letters.push(letter),
            Some(false) => kept.push(letter),
            None => gone.push(letter),
        }
    }
    ARRIVED.with_borrow_mut(|arrived| {
        kept.append(arrived);
        *arrived = kept;
    });
    // Dropped once the list is put back, so a letter whose drop posts another finds it.
    drop(gone);
    letters
}
