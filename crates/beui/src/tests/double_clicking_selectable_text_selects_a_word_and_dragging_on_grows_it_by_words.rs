use super::*;
use crate::reactive::{List, NodeRef, Text, build, view};
use crate::styled::SelectableText;

#[test]
fn double_clicking_selectable_text_selects_a_word_and_dragging_on_grows_it_by_words() {
    let (first, second, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (first, second, region) = (first.clone(), second.clone(), region.clone());
        move || {
            view! {
                <SelectableText @node_ref=&region>
                    <List spacing=4.0>
                        <Text @node_ref=&first string="alpha beta gamma" />
                        <Text @node_ref=&second string="delta epsilon" />
                    </List>
                </SelectableText>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let region = region.get();
    let (first, bottom) = (
        kind_of::<TextNode>(harness.document(), first.get()),
        harness.rect(second.get()),
    );
    let at = |harness: &Harness, index: usize| {
        let caret = harness
            .document()
            .text_caret_rect(first, index, 2.0)
            .expect("the text has a place for every index");
        pos2(caret.min.x + 1.0, caret.center().y)
    };
    let selected = |harness: &Harness| unstyled::selectable_text(harness.document(), region);
    let on_beta = at(&harness, "alpha be".len());

    harness.click(on_beta);
    harness.click(on_beta);
    assert_eq!(
        selected(&harness),
        "beta",
        "a double click selects the word"
    );

    harness.advance(Duration::from_secs(1));
    harness.click(on_beta);
    harness.frame(vec![Event::PointerButton {
        pos: on_beta,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(at(
        &harness,
        "alpha beta ga".len(),
    ))]);
    assert_eq!(
        selected(&harness),
        "beta gamma",
        "dragging on from a double click selects whole words"
    );
    let on_alpha = at(&harness, "al".len());
    harness.frame(vec![Event::PointerMoved(on_alpha)]);
    assert_eq!(
        selected(&harness),
        "alpha beta",
        "and keeps the first word when dragging back past it"
    );
    harness.release_at(on_alpha);

    harness.advance(Duration::from_secs(1));
    let on_delta = pos2(bottom.left() + 4.0, bottom.center().y);
    for _ in 0..3 {
        harness.click(on_delta);
    }
    assert_eq!(
        selected(&harness),
        "delta epsilon",
        "a triple click selects the whole text"
    );
}
