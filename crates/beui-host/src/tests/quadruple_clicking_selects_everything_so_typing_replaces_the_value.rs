use super::*;
use crate::styled::*;

#[test]
fn quadruple_clicking_selects_everything_so_typing_replaces_the_value() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let spot = pos2(rect.left() + 12.0, rect.center().y);
    for _ in 0..4 {
        harness.click(spot);
    }
    harness.type_text("bye");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "bye"
    );
}
