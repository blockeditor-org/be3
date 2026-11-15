use super::executor::{Answer, install, run_futures};
use super::*;
use crate::{AsyncMemo, ErrorBoundary, Pending, create_async_memo};

type Answers = Rc<RefCell<Vec<Answer<Result<u32, String>>>>>;

fn asking(answers: &Answers) -> AsyncMemo<u32> {
    let made = answers.clone();
    create_async_memo(move || {
        let answer = Answer::default();
        made.borrow_mut().push(answer.clone());
        async move { answer.await.map_err(Pending::Failed) }
    })
}

#[test]
fn a_reset_asks_again_only_what_failed_under_its_errored() {
    let _root = RootScope::new();
    install();
    // One async memo is made outside both boundaries and read under the first through a memo;
    // one is made under each.
    let outside_answers = Answers::default();
    let outside = asking(&outside_answers);
    let first_answers = Answers::default();
    let second_answers = Answers::default();
    let (first, second) = (Scope::new(), Scope::new());
    let first = first.run(|| {
        let errored = ErrorBoundary::provide();
        let made = asking(&first_answers);
        let doubled = create_memo(move || outside.get().map(|value| value * 2));
        create_effect(move || {
            let _ = (made.get(), doubled.get());
        });
        errored
    });
    let second = second.run(|| {
        let errored = ErrorBoundary::provide();
        let made = asking(&second_answers);
        create_effect(move || {
            let _ = made.get();
        });
        errored
    });
    flush();
    for answers in [&outside_answers, &first_answers, &second_answers] {
        answers.borrow()[0].give(Err("offline".to_owned()));
    }
    run_futures();
    assert_eq!(first.failure(), Some("offline".to_owned()));
    assert_eq!(second.failure(), Some("offline".to_owned()));

    first.reset();
    flush();
    assert_eq!(first_answers.borrow().len(), 2, "made under it: asked again");
    assert_eq!(outside_answers.borrow().len(), 2, "failed and read under it: asked again");
    assert_eq!(second_answers.borrow().len(), 1, "under the other Errored: left alone");
    assert_eq!(second.failure(), Some("offline".to_owned()));
}
