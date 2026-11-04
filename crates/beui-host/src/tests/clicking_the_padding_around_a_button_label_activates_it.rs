use super::*;
use crate::reactive::*;

#[test]
fn clicking_the_padding_around_a_button_label_activates_it() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                on_click={move || clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);

    harness.click(pos2(4.0, 4.0));

    assert_eq!(clicks.get_untracked(), 1);
}
