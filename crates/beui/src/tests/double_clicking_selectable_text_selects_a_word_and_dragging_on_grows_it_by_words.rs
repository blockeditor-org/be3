use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn double_clicking_selectable_text_selects_a_word_and_dragging_on_grows_it_by_words() {
    let _root = crate::reactive::RootScope::new();
    let (first, second, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=4.0>
                    <Text @node_ref=&first>"alpha beta gamma"</Text>
                    <Text @node_ref=&second>"delta epsilon"</Text>
                </List>
            </SelectableText>
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
    let selected = |harness: &mut Harness| selected_text(harness, region);
    let on_beta = at(&harness, "alpha be".len());

    harness.click(on_beta);
    harness.click(on_beta);
    assert_eq!(
        selected(&mut harness),
        "beta",
        "a double click selects the word"
    );

    harness.advance(Duration::from_secs(1));
    harness.click(on_beta);
    harness.frame(vec![Input::PointerButton {
        pos: on_beta,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerMoved(at(
        &harness,
        "alpha beta ga".len(),
    ))]);
    assert_eq!(
        selected(&mut harness),
        "beta gamma",
        "dragging on from a double click selects whole words"
    );
    let on_alpha = at(&harness, "al".len());
    harness.frame(vec![Input::PointerMoved(on_alpha)]);
    assert_eq!(
        selected(&mut harness),
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
        selected(&mut harness),
        "delta epsilon",
        "a triple click selects the whole text"
    );
}
