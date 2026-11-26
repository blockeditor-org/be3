use super::executor::install;
use super::*;
use crate::{AsyncMemo, create_async_memo};

#[test]
fn an_async_memo_whose_source_reads_itself_is_a_cycle() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let itself = Rc::new(Cell::new(None::<AsyncMemo<u32>>));
    let reads = itself.clone();
    let memo = create_async_memo(move || {
        let id = id.get();
        let before = reads.get().map(|memo| memo.get_untracked());
        async move { Ok(id + before.and_then(Result::ok).unwrap_or(0)) }
    });
    itself.set(Some(memo));
    id.set(2);
    let read = catch_unwind(AssertUnwindSafe(|| memo.get_untracked()));
    let error = read.expect_err("reading itself while it asks is a cycle");
    let message = error
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| error.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    assert!(message.contains("cycle"), "{message}");
}
