use super::*;
use crate::enter_zone;

#[test]
fn an_effect_keeps_its_zone_for_the_effects_it_creates() {
    let _root = RootScope::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let outer = create_signal(0);
    let inner = create_signal(0);
    let scope = Scope::new();
    {
        let _zone = enter_zone(1);
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || {
                outer.get();
                let seen = seen.clone();
                create_effect(move || seen.borrow_mut().push(inner.get()));
            });
        });
    }
    outer.set(1);
    flush();
    seen.borrow_mut().clear();

    {
        let _zone = enter_zone(2);
        inner.set(9);
        flush();
    }
    assert!(seen.borrow().is_empty());

    drop(enter_zone(1));
    assert_eq!(*seen.borrow(), vec![9]);
}
