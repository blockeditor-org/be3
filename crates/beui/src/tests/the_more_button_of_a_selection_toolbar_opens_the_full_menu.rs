use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn the_more_button_of_a_selection_toolbar_opens_the_full_menu() {
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
    let on_hello = pos2(rect.left() + 4.0, rect.center().y);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    assert_eq!(selected_text(&mut harness, region), "hello");

    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_some(),
        "a double tap shows the toolbar"
    );
    let more = harness.center(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .more(),
    );
    harness.advance(Duration::from_secs(1));
    harness.touch(TouchPhase::Start, more);
    harness.touch(TouchPhase::End, more);

    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .open(harness.document()),
        "More opens the full menu"
    );
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_none(),
        "in place of the toolbar"
    );
    let menu = harness
        .document()
        .api::<unstyled::ContextMenuApi>(region)
        .menu();
    let select_all = harness.center(
        harness
            .document()
            .api::<unstyled::MenuListApi>(menu)
            .row_button(1),
    );
    harness.touch(TouchPhase::Start, select_all);
    harness.touch(TouchPhase::End, select_all);
    assert_eq!(selected_text(&mut harness, region), "hello world");
}
