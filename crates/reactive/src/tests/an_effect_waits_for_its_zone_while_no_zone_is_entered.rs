use super::*;
use crate::{enter_zone, zone_pending};

#[test]
fn an_effect_waits_for_its_zone_while_no_zone_is_entered() {
    let seen = Rc::new(Cell::new(0));
    let (count, set_count) = create_signal(0);
    let scope = Scope::new();
    {
        let _zone = enter_zone(3);
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || seen.set(count.get()));
        });
    }

    set_count.set(5);
    assert_eq!(seen.get(), 0);
    assert!(zone_pending(3));

    let _zone = enter_zone(3);
    assert_eq!(seen.get(), 5);
}
