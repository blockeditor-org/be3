use super::*;
use crate::primitives::*;

const PARAGRAPH: &str = "A paragraph long enough to wrap onto more lines once the view it is shown in grows narrower than it was when it was last laid out.";
const FAR_BELOW: f32 = 5000.0;

#[test]
fn a_culled_text_narrowed_after_it_was_shaped_does_not_overflow() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <Frame height=FAR_BELOW />
                        <Text @node_ref=&text wrap=true>{PARAGRAPH}</Text>
                    </List>
                </Offset>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(800.0, FAR_BELOW + 200.0));
    harness.frame(Vec::new());
    assert!(
        !harness.document().is_culled(text.get()),
        "the text is drawn"
    );

    harness.resize(Vec2::new(300.0, 400.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().is_culled(text.get()),
        "the text is laid out far below the view"
    );
    let found: Vec<String> = harness
        .document()
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found.is_empty(),
        "a culled text is not drawn, so the wrap it was last drawn with cannot spill out of its new box: {found:#?}"
    );
}
