use super::*;
use crate::reactive::view;

#[test]
fn a_missing_reactive_prop_panics_instead_of_defaulting() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build(|| {
            view! {
                <Text />
            }
        })
    }));
    std::panic::set_hook(previous);

    let Err(panic) = result else {
        panic!("a missing `Prop` prop is not filled in with a default");
    };
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .expect("the panic carries its message");
    assert_eq!(
        message,
        "missing required prop `string` for component `Text`"
    );
}
