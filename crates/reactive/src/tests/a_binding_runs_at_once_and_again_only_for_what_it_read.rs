use super::*;
use crate::create_binding;

#[test]
fn a_binding_runs_at_once_and_again_only_for_what_it_read() {
    let _root = RootScope::new();
    let (tracked, set_tracked) = create_signal(0);
    let (ignored, set_ignored) = create_signal(0);
    let doubled = create_memo(move || tracked.get() * 2);
    let seen = Rc::new(RefCell::new(Vec::new()));

    batch({
        let seen = seen.clone();
        move || {
            let pushed = seen.clone();
            create_binding(move || {
                let ignored = untrack(|| ignored.get());
                pushed.borrow_mut().push((doubled.get(), ignored));
            });
            assert_eq!(
                *seen.borrow(),
                vec![(0, 0)],
                "a binding runs before the batch around it ends"
            );
        }
    });

    set_ignored.set(1);
    assert_eq!(
        *seen.borrow(),
        vec![(0, 0)],
        "what it read untracked does not wake it"
    );
    set_tracked.set(1);
    assert_eq!(*seen.borrow(), vec![(0, 0), (2, 1)]);
}
