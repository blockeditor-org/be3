use super::*;
use crate::reactive::view;
use crate::styled::{NumberInput, number_input_text};

#[test]
fn a_number_input_shows_its_value_to_the_decimals_asked() {
    let _root = crate::reactive::RootScope::new();
    let (document, [rounded, exact, whole]) = toolbar_of(move || {
        [
            view! {
                <NumberInput value=182.2222137451172 decimals={Some(2)} on_change={|_| {}} />
            },
            view! {
                <NumberInput value=182.2222137451172 on_change={|_| {}} />
            },
            view! {
                <NumberInput value=-0.001 decimals={Some(2)} on_change={|_| {}} />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let shown = |input| {
        let face = number_input_text(harness.document(), input).expect("the input starts closed");
        text_of(harness.document(), face)
    };
    assert_eq!(shown(rounded), "182.22");
    assert_eq!(shown(exact), "182.2222137451172");
    assert_eq!(shown(whole), "0");
}
