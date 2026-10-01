use super::*;
use crate::enter_zone;

#[test]
fn effects_of_every_zone_run_when_no_zone_is_entered() {
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

    assert_eq!(seen.get(), 5);
}
