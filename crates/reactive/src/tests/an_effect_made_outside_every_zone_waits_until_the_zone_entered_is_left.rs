use super::*;
use crate::{enter_zone, zone_pending};

#[test]
fn an_effect_made_outside_every_zone_waits_until_the_zone_entered_is_left() {
    let _root = RootScope::new();
    let seen = Rc::new(Cell::new(0));
    let count = create_signal(1);
    {
        let seen = seen.clone();
        create_effect(move || seen.set(count.get()));
    }
    flush();
    assert_eq!(seen.get(), 1);

    count.set(2);
    flush();
    assert_eq!(seen.get(), 2, "flushed outside every zone, it runs");

    {
        let _zone = enter_zone(5);
        count.set(3);
        flush();
        assert_eq!(seen.get(), 2);
        assert!(zone_pending(0));
    }
    assert_eq!(seen.get(), 3);
    assert!(!zone_pending(0));
}
