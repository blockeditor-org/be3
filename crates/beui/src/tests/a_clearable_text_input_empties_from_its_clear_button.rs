use super::*;
use crate::icons::ICON_SEARCH;
use crate::reactive::{create_signal, view};
use crate::styled::TextInput;

#[test]
fn a_clearable_text_input_empties_from_its_clear_button() {
    let (value, set_value) = create_signal("road".to_owned());
    let shown = value.clone();
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <TextInput
                value={shown.clone()}
                glyph={ICON_SEARCH.to_owned()}
                clearable=true
                on_change={move |next: String| set_value.set(next)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    harness.click(Pos2::new(rect.right() - 14.0, rect.center().y));
    harness.frame(Vec::new());

    assert_eq!(value.get_untracked(), "");
}
