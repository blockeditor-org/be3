use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_single_line_input_longer_than_its_field_scrolls_rather_than_overflows() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal("a link far longer than the field it is shown in, which scrolls across to show the rest of it".to_owned());
    let (document, [_input]) = toolbar_of(move || {
        [view! {
            <TextInput value={value} />
        }]
    });
    let mut harness = Harness::sized(document, Vec2::new(200.0, 100.0));
    harness.frame(Vec::new());

    let found: Vec<String> = harness
        .document()
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}
