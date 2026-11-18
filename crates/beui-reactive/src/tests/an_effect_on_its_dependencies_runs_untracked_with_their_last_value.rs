use super::*;
use crate::create_effect_on;

#[test]
fn an_effect_on_its_dependencies_runs_untracked_with_their_last_value() {
    let _root = RootScope::new();
    let count = create_signal(1);
    let other = create_signal(10);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let recorded = seen.clone();
    create_effect_on(
        move || count.get(),
        move |count, previous| {
            recorded
                .borrow_mut()
                .push((*count, previous.copied(), other.get()));
        },
    );
    flush();
    other.set(11);
    flush();
    assert_eq!(
        *seen.borrow(),
        [(1, None, 10)],
        "what the body reads does not wake it"
    );
    count.set(2);
    flush();
    assert_eq!(*seen.borrow(), [(1, None, 10), (2, Some(1), 11)]);
}
