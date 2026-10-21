use super::*;
use crate::{enter_zone, forget_zone, wake_zone_with};

#[test]
fn a_zone_is_woken_once_when_its_first_effect_waits() {
    let _root = RootScope::new();
    let woken = Rc::new(Cell::new(0));
    let seen = Rc::new(Cell::new(0));
    let count = create_signal(0);
    let other = create_signal(0);
    let scope = Scope::new();
    {
        let _zone = enter_zone(11);
        let seen = seen.clone();
        scope.run(move || {
            create_effect(move || seen.set(count.get()));
            create_effect(move || {
                other.get();
            });
        });
    }
    wake_zone_with(11, {
        let woken = woken.clone();
        move || woken.set(woken.get() + 1)
    });

    count.set(1);
    flush();
    other.set(1);
    flush();
    count.set(2);
    flush();
    assert_eq!(woken.get(), 1);
    assert_eq!(seen.get(), 0);

    {
        let _zone = enter_zone(11);
        assert_eq!(seen.get(), 2);
        count.set(3);
        flush();
        assert_eq!(seen.get(), 3);
    }
    assert_eq!(woken.get(), 1);

    count.set(4);
    flush();
    assert_eq!(woken.get(), 2);

    forget_zone(11);
    {
        let _zone = enter_zone(11);
        assert_eq!(seen.get(), 3);
    }
    count.set(5);
    flush();
    assert_eq!(woken.get(), 2);
}
