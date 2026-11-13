use super::*;

use be_block_commit::CommitId;

use crate::search::{Unwritten, WRITE_ATTEMPTS, on_newer_heads};

fn commit(n: u8) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::from_bytes([n; 32]))
}

#[test]
fn a_batch_whose_index_head_keeps_moving_is_given_back_after_a_few_tries() {
    // Every head tried is gone by the time it is read, and the server always names a newer one.
    let tried = std::cell::RefCell::new(Vec::new());
    let next = std::cell::Cell::new(1);
    let written: Result<(), Unwritten> = runtime().block_on(on_newer_heads(
        commit(0),
        |head| {
            tried.borrow_mut().push(head);
            async { Err(Unwritten::Moved("gone".to_owned())) }
        },
        || {
            let head = commit(next.get());
            next.set(next.get() + 1);
            async move { Some(head) }
        },
    ));
    assert_eq!(written, Err(Unwritten::Moved("gone".to_owned())));
    assert_eq!(
        *tried.borrow(),
        (0..WRITE_ATTEMPTS as u8).map(commit).collect::<Vec<_>>(),
        "each head the server named was tried once, up to the cap"
    );

    // A head that is gone with no newer one to move to is given back at once, as is any other
    // failure.
    let attempts = std::cell::Cell::new(0);
    let written: Result<(), Unwritten> = runtime().block_on(on_newer_heads(
        commit(0),
        |_| {
            attempts.set(attempts.get() + 1);
            async { Err(Unwritten::Moved("gone".to_owned())) }
        },
        || async { None },
    ));
    assert_eq!(written, Err(Unwritten::Moved("gone".to_owned())));
    assert_eq!(attempts.get(), 1);
    let written: Result<(), Unwritten> = runtime().block_on(on_newer_heads(
        commit(0),
        |_| async { Err(Unwritten::Failed("refused".to_owned())) },
        || async { Some(commit(1)) },
    ));
    assert_eq!(written, Err(Unwritten::Failed("refused".to_owned())));
}
