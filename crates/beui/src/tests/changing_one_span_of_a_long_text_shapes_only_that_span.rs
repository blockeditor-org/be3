use super::*;
use crate::primitives::Span;
use crate::primitives::*;
use crate::reactive::*;

const WORDS: usize = 60;

fn words() -> Vec<String> {
    (0..WORDS).map(|index| format!("word{index} ")).collect()
}

#[test]
fn changing_one_span_of_a_long_text_shapes_only_that_span() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let writer = create_signal::<Option<Signal<Vec<String>>>>(None);
    let document = build_node(move || {
        let words = create_signal(words());
        writer.update(|writer| *writer = Some(words));
        let keys = create_memo(move || (0..words.get().len()).collect::<Vec<usize>>());
        view! {
            <Frame width=200.0>
                <Text @node_ref=&text>
                    <ForEach keys={keys}>|index: usize| {
                        let word = create_memo(move || {
                            words.get().get(index).cloned().unwrap_or_default()
                        });
                        view! {
                            <Span>{word}</Span>
                        }
                    }</ForEach>
                </Text>
            </Frame>
        }
    });
    let written = writer.get_untracked().expect("the words were made");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = kind_of::<TextNode>(harness.document(), text.get());
    let layout = harness
        .document()
        .text_layout(text)
        .expect("the text was laid out");
    assert!(layout.lines.len() > 3, "the text wraps onto many lines");
    let shaped = harness.document().text_shapings(text);
    assert!(
        shaped >= WORDS as u64,
        "every word was shaped once at first"
    );

    let mut changed = words();
    changed[WORDS / 2] = "changed ".to_owned();
    enter(harness.document_mut(), || written.set(changed.clone()));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().text_shapings(text) - shaped,
        1,
        "only the span that changed is shaped again"
    );
    assert!(
        harness
            .document()
            .text(text)
            .contains("word29 changed word31"),
        "the text reads the changed span in its place"
    );

    let shaped = harness.document().text_shapings(text);
    changed.push("added".to_owned());
    enter(harness.document_mut(), || written.set(changed.clone()));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().text_shapings(text) - shaped,
        1,
        "a span that arrives is the only one shaped"
    );
    assert!(harness.document().text(text).ends_with("word59 added"));
}
