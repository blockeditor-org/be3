use super::*;
use crate::icons::ICON_SEARCH;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_clearable_text_input_empties_from_its_clear_button() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal("road".to_owned());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <TextInput value={value} glyph=ICON_SEARCH clearable=true />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    harness.click(Pos2::new(rect.right() - 14.0, rect.center().y));
    harness.frame(Vec::new());

    assert_eq!(value.get_untracked(), "");
}
