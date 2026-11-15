//! On the web the page takes no lock another thread takes (plans/web_page_never_waits.md), so
//! a letter waits in a list of the page's own. One the page posts to itself goes straight onto
//! it. One a worker posts is handed to the page by `postMessage` (`web/threads.js`,
//! "be-letter-post"), and the page's handler puts it on the list (`be_letter_arrived`) and
//! wakes the host. A worker's letters reach the page in the order it posted them (each thread
//! posts on one channel); letters from different threads arrive in no set order between them,
//! and a worker's letter arrives a task after it was posted, not at once.
//!
//! Only the page makes mailboxes: it is the one thread whose frames deliver them.

use std::{cell::RefCell, thread, thread::ThreadId};

use wasm_bindgen::prelude::wasm_bindgen;

use super::{Letter, wake};

thread_local! {
    static ARRIVED: RefCell<Vec<Letter>> = const { RefCell::new(Vec::new()) };
}

// unsafe(ffi)
#[link(wasm_import_module = "wasi")]
unsafe extern "C" {
    #[link_name = "be-letter-post"]
    fn be_letter_post(letter: usize);
}

pub(super) fn post(to: ThreadId, letter: Letter) {
    if to == thread::current().id() {
        arrived(letter);
        return;
    }
    // The page frees it (`be_letter_arrived`): until plans/web_page_never_waits.md gives the
    // stack's worker a memory of its own, the page and the worker run in one.
    let letter = Box::into_raw(Box::new(letter)) as usize;
    // unsafe(ffi)
    unsafe { be_letter_post(letter) };
}

fn arrived(letter: Letter) {
    ARRIVED.with_borrow_mut(|arrived| arrived.push(letter));
    wake();
}

/// A letter a worker posted (`post`), handed over by the page's handler for its message.
#[wasm_bindgen]
pub fn be_letter_arrived(letter: usize) {
    // unsafe(memory)
    let letter = unsafe { Box::from_raw(letter as *mut Letter) };
    arrived(*letter);
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
