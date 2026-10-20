use super::*;
use crate::reactive::Binding;

#[test]
fn a_derived_binding_reads_its_prop_and_writes_through_its_callback() {
    let _root = ::reactive::RootScope::new();
    let celsius = create_signal(100.0_f32);
    let fahrenheit = Binding::new(
        Prop::dynamic(move || celsius.get() * 9.0 / 5.0 + 32.0),
        move |value: f32| celsius.set((value - 32.0) * 5.0 / 9.0),
    );
    assert_eq!(fahrenheit.get_untracked(), 212.0);
    let writes = Rc::new(Cell::new(0));
    create_effect({
        let writes = writes.clone();
        move || {
            celsius.get();
            writes.set(writes.get() + 1);
        }
    });
    fahrenheit.set(212.0);
    assert_eq!(writes.get(), 1);
    fahrenheit.set(32.0);
    assert_eq!(celsius.get_untracked(), 0.0);
    assert_eq!(writes.get(), 2);
    fahrenheit.update(|value| *value += 18.0);
    assert_eq!(celsius.get_untracked(), 10.0);
}
