use super::*;
use crate::styled::*;

#[test]
fn triple_clicking_selects_the_line_so_typing_replaces_the_value() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let spot = pos2(rect.left() + 12.0, rect.center().y);
    harness.click(spot);
    harness.click(spot);
    harness.click(spot);
    harness.type_text("bye");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "bye"
    );
}
