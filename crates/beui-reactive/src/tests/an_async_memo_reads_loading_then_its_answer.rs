use super::executor::{Answer, install, run_futures};
use super::*;
use crate::{Pending, create_async_memo};

#[test]
fn an_async_memo_reads_loading_then_its_answer() {
    let _root = RootScope::new();
    install();
    let answer = Answer::<u32>::default();
    let asked = answer.clone();
    let memo = create_async_memo(move || {
        let asked = asked.clone();
        async move { Ok(asked.await) }
    });
    assert_eq!(memo.get_untracked(), Err(Pending::Loading));
    assert!(memo.pending());
    answer.give(4);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(4));
    assert!(!memo.pending());
}
