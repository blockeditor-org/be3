use super::*;

#[test]
fn a_selector_forgets_keys_whose_watchers_were_disposed() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let selected = create_signal(0usize);
    let selector = scope.run(|| create_selector(move || selected.get()));
    let watch_left = create_signal(true);
    let rows = scope.run(Scope::new);
    rows.run({
        move || {
            for index in 0..3 {
                let selector = selector;
                create_effect(move || {
                    selector.is_selected(&index);
                });
            }
            create_effect(move || {
                selector.is_selected(&if watch_left.get() { 0 } else { 9 });
            });
        }
    });
    assert_eq!(selector.watched_keys(), 3);
    watch_left.set(false);
    assert_eq!(selector.watched_keys(), 4);
    watch_left.set(true);
    assert_eq!(selector.watched_keys(), 3);
    rows.dispose();
    assert_eq!(selector.watched_keys(), 0);
    selected.set(1);
    assert_eq!(selector.watched_keys(), 0);
}
