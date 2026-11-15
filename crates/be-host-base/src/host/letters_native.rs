//! Natively a letter waits in one list every thread locks, until the thread that made its
//! mailbox takes it.

use std::{
    sync::{Condvar, Mutex, PoisonError},
    thread::ThreadId,
    time::Duration,
};

use super::{Encoded, Letter, wake};

static POSTED: Mutex<Vec<(ThreadId, Letter)>> = Mutex::new(Vec::new());
/// Told whenever a letter is posted (`wait_for_letter`).
static POSTED_SIGNAL: Condvar = Condvar::new();
/// How many letters were ever posted, which `wait_for_letter` waits to see grow.
static POSTED_COUNT: Mutex<u64> = Mutex::new(0);

pub(super) fn post(to: ThreadId, letter: Letter) {
    POSTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((to, letter));
    *POSTED_COUNT.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    POSTED_SIGNAL.notify_all();
    wake();
}

/// A letter in bytes, which its mailbox decodes as it delivers it (`Across`).
pub(super) fn post_bytes(to: ThreadId, mailbox: u64, bytes: Vec<u8>) {
    post(to, (mailbox, Box::new(Encoded(bytes))));
}

/// Takes, in the order they were posted, the letters for `here` whose mailbox `at_this_desk`
/// answers true for, and drops those whose mailbox is gone (`None`).
pub(super) fn take(here: ThreadId, at_this_desk: impl Fn(u64) -> Option<bool>) -> Vec<Letter> {
    let mut posted = POSTED.lock().unwrap_or_else(PoisonError::into_inner);
    let mut letters = Vec::new();
    posted.retain_mut(|(thread, letter)| {
        if *thread != here {
            return true;
        }
        match at_this_desk(letter.0) {
            Some(true) => {
                letters.push(std::mem::replace(letter, (0, Box::new(()))));
                false
            }
            Some(false) => true,
            None => false,
        }
    });
    letters
}

/// How many letters were ever posted: what a caller hands `wait_for_letter` to wait past.
pub fn letters_posted() -> u64 {
    *POSTED_COUNT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Waits until a letter is posted after `seen` (`letters_posted`), up to `limit`, and answers
/// whether one was: for a test that waits on work another thread answers through a mailbox.
/// It takes no letter; a desk's `deliver` does.
pub fn wait_for_letter(seen: u64, limit: Duration) -> bool {
    let count = POSTED_COUNT.lock().unwrap_or_else(PoisonError::into_inner);
    let (_count, waited) = POSTED_SIGNAL
        .wait_timeout_while(count, limit, |count| *count == seen)
        .unwrap_or_else(PoisonError::into_inner);
    !waited.timed_out()
}
