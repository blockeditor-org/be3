use super::*;

#[test]
fn a_resumed_effect_runs_when_what_it_read_changed_and_changed_back() {
    let _root = RootScope::new();
    let focused = create_signal(true);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let scope = Scope::detached().pausable();
    scope.run({
        let seen = seen.clone();
        move || {
            let wanted = create_memo(move || focused.get());
            create_effect(move || seen.borrow_mut().push(wanted.get()));
        }
    });
    assert_eq!(*seen.borrow(), vec![true]);

    scope.pause();
    focused.set(false);
    flush();
    focused.set(true);
    flush();
    assert_eq!(*seen.borrow(), vec![true], "a paused scope's effects wait");

    scope.resume();
    flush();
    assert_eq!(
        *seen.borrow(),
        vec![true, true],
        "what it read moved while it waited, so it applies the current value again on resume, \
         even though the memo it reads came back to what it was"
    );

    scope.pause();
    scope.resume();
    flush();
    assert_eq!(
        *seen.borrow(),
        vec![true, true],
        "a pause in which nothing it read changed runs nothing"
    );
}
