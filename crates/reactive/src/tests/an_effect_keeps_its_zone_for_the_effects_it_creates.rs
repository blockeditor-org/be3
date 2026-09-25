use super::*;
use crate::enter_zone;

#[test]
fn an_effect_keeps_its_zone_for_the_effects_it_creates() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (outer, set_outer) = create_signal(0);
    let (inner, set_inner) = create_signal(0);
    let scope = Scope::new();
    {
        let _zone = enter_zone(1);
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || {
                outer.get();
                let seen = seen.clone();
                let inner = inner.clone();
                create_effect(move || seen.borrow_mut().push(inner.get()));
            });
        });
    }
    set_outer.set(1);
    seen.borrow_mut().clear();

    {
        let _zone = enter_zone(2);
        set_inner.set(9);
    }
    assert!(seen.borrow().is_empty());

    drop(enter_zone(1));
    assert_eq!(*seen.borrow(), vec![9]);
}
