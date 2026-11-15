use super::executor::{Answer, install, run_futures, spawned};
use super::*;
use crate::create_async_memo;

#[test]
fn a_rerun_async_memo_drops_its_last_future_and_keeps_its_last_answer() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let answers = Rc::new(RefCell::new(Vec::<Answer<u32>>::new()));
    let made = answers.clone();
    let memo = create_async_memo(move || {
        let id = id.get();
        let answer = Answer::default();
        made.borrow_mut().push(answer.clone());
        async move { Ok(answer.await * 10 + id) }
    });
    answers.borrow()[0].give(1);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(11));

    id.set(2);
    flush();
    assert_eq!(memo.get_untracked(), Ok(11), "the last answer shows while the next is asked");
    assert!(memo.pending());
    id.set(3);
    flush();
    run_futures();
    assert_eq!(spawned(), 1, "the dropped future finished without an answer");
    answers.borrow()[1].give(5);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(11), "a dropped future's answer never lands");
    answers.borrow()[2].give(2);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(23));
    assert!(!memo.pending());
}
