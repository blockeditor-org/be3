use super::executor::{Answer, install, run_futures};
use super::*;
use crate::create_async_memo;

#[test]
fn an_answer_landing_after_its_async_memo_is_due_again_is_dropped() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let answers = Rc::new(RefCell::new(Vec::<Answer<u32>>::new()));
    let made = answers.clone();
    let memo = create_async_memo(move || {
        let id = id.get();
        let answer = Answer::default();
        made.borrow_mut().push(answer.clone());
        async move { Ok(answer.await + id) }
    });
    // The first future answers after the id changed, before the memo ran again: its answer is
    // for the id before, so it does not land.
    id.set(2);
    answers.borrow()[0].give(10);
    run_futures();
    assert!(
        memo.get_untracked().is_err(),
        "the answer for the old id is dropped"
    );
    answers.borrow()[1].give(20);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(22));
}
