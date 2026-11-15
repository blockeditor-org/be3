use super::executor::{Answer, install, run_futures};
use super::*;
use crate::{create_async_memo, create_trigger};

#[test]
fn an_answer_for_an_old_memo_value_is_dropped_and_one_for_an_equal_value_lands() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let half = create_memo(move || id.get() / 2);
    let answers = Rc::new(RefCell::new(Vec::<Answer<u32>>::new()));
    let again = create_trigger();
    let made = answers.clone();
    let memo = create_async_memo(move || {
        again.track();
        let half = half.get();
        let answer = Answer::default();
        made.borrow_mut().push(answer.clone());
        async move { Ok(answer.await + half) }
    });
    // The memo it read keeps its value: the answer is still for what it reads, so it lands.
    id.set(0);
    answers.borrow()[0].give(10);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(10));
    assert_eq!(answers.borrow().len(), 1, "an equal memo value does not ask again");
    // The memo it read changes, and the answer lands before the async memo runs again: the
    // answer is for the old value, so it is dropped.
    again.notify();
    run_futures();
    id.set(4);
    answers.borrow()[1].give(30);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(10), "the answer for the old value is dropped");
    answers.borrow()[2].give(20);
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(22));
}
