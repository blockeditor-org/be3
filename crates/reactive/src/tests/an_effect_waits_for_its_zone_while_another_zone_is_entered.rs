use super::*;
use crate::{enter_zone, zone_pending};

#[test]
fn an_effect_waits_for_its_zone_while_another_zone_is_entered() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (count, set_count) = create_signal(0);
    let scope = Scope::new();
    {
        let _zone = enter_zone(7);
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || seen.borrow_mut().push(count.get()));
        });
    }
    assert_eq!(*seen.borrow(), vec![0]);

    {
        let _zone = enter_zone(8);
        set_count.set(1);
        assert_eq!(*seen.borrow(), vec![0]);
    }
    assert!(zone_pending(7));

    {
        let _zone = enter_zone(7);
        assert_eq!(*seen.borrow(), vec![0, 1]);
    }
    assert!(!zone_pending(7));
}
