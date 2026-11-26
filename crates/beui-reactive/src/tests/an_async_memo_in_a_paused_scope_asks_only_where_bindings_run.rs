use super::executor::{Answer, install, run_futures};
use super::*;
use crate::{Boundary, Pending, create_async_memo};

/// A `Loading`'s hidden content pauses only effects, so an async memo in it asks at once and
/// its readers wait; a scope paused whole holds the async memo too until it resumes.
#[test]
fn an_async_memo_in_a_paused_scope_asks_only_where_bindings_run() {
    let _root = RootScope::new();
    install();
    for effects_only in [true, false] {
        let id = create_signal(1_u32);
        let answers = Rc::new(RefCell::new(Vec::<Answer<u32>>::new()));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let scope = match effects_only {
            true => Scope::detached().pausable_effects(),
            false => Scope::detached().pausable(),
        };
        let (loading, memo) = scope.run({
            let answers = answers.clone();
            let seen = seen.clone();
            move || {
                let loading = Boundary::provide();
                let memo = create_async_memo(move || {
                    let id = id.get();
                    let answer = Answer::default();
                    answers.borrow_mut().push(answer.clone());
                    async move { Ok(answer.await * id) }
                });
                create_effect(move || seen.borrow_mut().push(memo.get()));
                (loading, memo)
            }
        });
        flush();
        assert_eq!(*seen.borrow(), vec![Err(Pending::Loading)]);

        scope.pause();
        id.set(2);
        flush();
        if !effects_only {
            assert_eq!(answers.borrow().len(), 1, "a scope paused whole holds it");
            scope.resume();
            flush();
            scope.pause();
        }
        assert_eq!(answers.borrow().len(), 2, "it asks for the new id");
        answers.borrow()[1].give(5);
        run_futures();
        assert_eq!(memo.get_untracked(), Ok(10), "and its answer lands");
        assert_eq!(loading.state(), Ok(()));
        let shown = seen.borrow().last().cloned();
        assert_eq!(shown, Some(Err(Pending::Loading)), "its readers wait");

        scope.resume();
        flush();
        assert_eq!(seen.borrow().last().cloned(), Some(Ok(10)));
        scope.dispose();
    }
}
