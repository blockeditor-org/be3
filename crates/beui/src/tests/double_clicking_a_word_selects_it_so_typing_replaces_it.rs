use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn double_clicking_a_word_selects_it_so_typing_replaces_it() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="hello world" />
        }]
    });
    let mut harness = harness(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let spot = pos2(rect.left() + 12.0, rect.center().y);
    harness.click(spot);
    harness.click(spot);
    harness.type_text("bye");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "bye world"
    );
}
