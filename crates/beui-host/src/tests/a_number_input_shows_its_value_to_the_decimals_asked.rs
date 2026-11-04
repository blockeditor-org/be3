use super::*;
use crate::styled::*;

#[test]
fn a_number_input_shows_its_value_to_the_decimals_asked() {
    let _root = crate::reactive::RootScope::new();
    let (document, [rounded, exact, whole, huge]) = toolbar_of(move || {
        [
            view! {
                <NumberInput value={182.2222137451172} decimals={Some(2)} />
            },
            view! {
                <NumberInput value={182.2222137451172} />
            },
            view! {
                <NumberInput value={-0.001} decimals={Some(2)} />
            },
            view! {
                <NumberInput value={1.5e300} decimals={Some(u32::MAX)} />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let shown = |input| {
        let face = harness
            .document()
            .api::<NumberInputTextApi>(input)
            .text()
            .expect("the input starts closed");
        text_of(harness.document(), face)
    };
    assert_eq!(shown(rounded), "182.22");
    assert_eq!(shown(exact), "182.2222137451172");
    assert_eq!(shown(whole), "0");
    assert_eq!(
        shown(huge),
        format!("{}", 1.5e300),
        "a huge value or decimals count is shown unrounded, never as NaN"
    );
}
