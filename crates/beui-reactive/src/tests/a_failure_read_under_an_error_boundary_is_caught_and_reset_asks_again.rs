use super::executor::{Answer, install, run_futures};
use super::*;
use crate::{ErrorBoundary, Pending, create_async_memo};

#[test]
fn a_failure_read_under_an_error_boundary_is_caught_and_reset_asks_again() {
    let _root = RootScope::new();
    install();
    let answers = Rc::new(RefCell::new(Vec::<Answer<Result<u32, String>>>::new()));
    let made = answers.clone();
    let scope = Scope::new();
    let (errored, memo) = scope.run(|| {
        let errored = ErrorBoundary::provide();
        let memo = create_async_memo(move || {
            let answer = Answer::default();
            made.borrow_mut().push(answer.clone());
            async move { answer.await.map_err(Pending::Failed) }
        });
        create_effect(move || memo.track());
        (errored, memo)
    });
    flush();
    assert_eq!(errored.failure(), None, "loading is no failure");

    answers.borrow()[0].give(Err("offline".to_owned()));
    run_futures();
    assert_eq!(errored.failure(), Some("offline".to_owned()));

    errored.reset();
    flush();
    assert_eq!(errored.failure(), None, "a reset asks again");
    assert_eq!(answers.borrow().len(), 2);
    answers.borrow()[1].give(Ok(7));
    run_futures();
    assert_eq!(memo.get_untracked(), Ok(7));
    assert_eq!(errored.failure(), None);
}
