use super::*;
use crate::{enter_zone, zone_pending};

#[test]
fn an_effect_woken_in_an_inner_zone_runs_when_its_own_zone_is_returned_to() {
    let _root = RootScope::new();
    let seen = Rc::new(Cell::new(0));
    let count = create_signal(0);
    let scope = Scope::new();
    let _outer = enter_zone(3);
    {
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || seen.set(count.get()));
        });
    }

    {
        let _inner = enter_zone(4);
        count.set(7);
        assert_eq!(seen.get(), 0);
        assert!(zone_pending(3));
    }
    assert_eq!(seen.get(), 7);
}
