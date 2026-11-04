use super::*;

#[test]
fn conditional_memos_replace_dependencies() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let choose_left = create_signal(true);
    let left = create_signal(1);
    let right = create_signal(2);
    let calls = Rc::new(Cell::new(0));
    let memo = scope.run(|| {
        let calls = calls.clone();
        create_memo(move || {
            calls.set(calls.get() + 1);
            if choose_left.get() {
                left.get()
            } else {
                right.get()
            }
        })
    });
    choose_left.set(false);
    assert_eq!(memo.get(), 2);
    left.set(3);
    assert_eq!(memo.get(), 2);
    assert_eq!(calls.get(), 2);
    right.set(4);
    assert_eq!(memo.get(), 4);
}
