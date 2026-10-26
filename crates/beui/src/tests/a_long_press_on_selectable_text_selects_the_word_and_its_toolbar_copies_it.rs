use super::*;
use crate::base::overlay::OverlayNode;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_long_press_on_selectable_text_selects_the_word_and_its_toolbar_copies_it() {
    let _root = crate::reactive::RootScope::new();
    let (text, region) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=4.0>
                    <Text @node_ref=&text>"hello world"</Text>
                </List>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (text, region) = (text.get(), region.get());
    let rect = harness.rect(text);
    let on_world = pos2(rect.right() - 4.0, rect.center().y);

    harness.touch(TouchPhase::Start, on_world);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, on_world);
    assert_eq!(
        selected_text(&mut harness, region),
        "world",
        "a long press selects the word under the finger"
    );
    assert!(
        !harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .open(harness.document()),
        "the long press does not open the full menu"
    );
    let toolbar = harness
        .document()
        .api::<unstyled::ContextMenuApi>(region)
        .toolbar(harness.document())
        .expect("it shows the toolbar instead");
    let bar = harness.rect(
        harness
            .document()
            .overlay_content(kind_of::<OverlayNode>(harness.document(), toolbar))
            .expect("the toolbar has content"),
    );
    assert!(
        bar.bottom() <= rect.top() || bar.top() >= rect.bottom(),
        "the toolbar floats clear of the selected text: {bar:?} and {rect:?}"
    );

    let copy = harness
        .center(text_within(harness.document(), toolbar, "Copy").expect("the toolbar offers Copy"));
    harness.touch(TouchPhase::Start, copy);
    let copied = harness.touch(TouchPhase::End, copy);
    assert_eq!(copied.copied_text.as_deref(), Some("world"));
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_none(),
        "choosing an action puts the toolbar away"
    );
    assert_eq!(
        selected_text(&mut harness, region),
        "world",
        "and keeps the selection"
    );
}
