use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_tap_outside_selectable_text_puts_its_toolbar_away() {
    let _root = crate::reactive::RootScope::new();
    let (text, region, elsewhere) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=40.0>
                <SelectableText @node_ref=&region>
                    <List spacing=4.0>
                        <Text @node_ref=&text>"hello world"</Text>
                    </List>
                </SelectableText>
                <Text @node_ref=&elsewhere>"somewhere else"</Text>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (text, region) = (harness.rect(text.get()), region.get());
    let on_hello = pos2(text.left() + 4.0, text.center().y);

    harness.touch(TouchPhase::Start, on_hello);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, on_hello);
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_some(),
        "a long press shows the toolbar"
    );

    harness.advance(Duration::from_secs(1));
    let away = harness.center(elsewhere.get());
    harness.touch(TouchPhase::Start, away);
    harness.touch(TouchPhase::End, away);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_none(),
        "a tap outside the text puts the toolbar away"
    );
}
