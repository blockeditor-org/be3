use super::*;
use crate::reactive::{
    ForEach, Frame, Memo, clone, component, create_memo, create_signal, view, with_reactive_scope,
};
use crate::styled::ContextMenu;
use crate::unstyled::MenuItem;

const REGION: f32 = 60.0;

#[test]
fn a_component_wrapping_a_node_less_component_keeps_its_scope() {
    let menu = NodeRef::new();
    let region = NodeRef::new();
    let (count, set_count) = create_signal(0usize);
    let document = crate::reactive::build({
        let menu = menu.clone();
        let region = region.clone();
        move || {
            view! {
                <Rows @node_ref=&menu region={region} count={count} />
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let pos = harness.center(region.get());
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_count.set(2));
    harness.frame(Vec::new());

    let content = unstyled::context_menu_menu(harness.document(), menu.get());
    assert_eq!(
        unstyled::menu_list_len(harness.document(), content),
        2,
        "a wrapper component contributes one item per row"
    );
    let labels: Vec<String> = (0..2)
        .map(|index| {
            let button = unstyled::menu_list_row_button(harness.document(), content, index);
            text_within(harness.document(), button).expect("a row shows its label")
        })
        .collect();
    assert_eq!(
        labels,
        vec!["row 0".to_owned(), "row 1".to_owned()],
        "the inner component's scope survives the wrapper adopting the value it returned"
    );
}

#[component]
fn Rows(region: NodeRef, count: crate::reactive::ReadSignal<usize>) -> NodeId {
    let keys = create_memo(clone!(count -> move || (0..count.get()).collect::<Vec<usize>>()));
    view! {
        <ContextMenu
            items={view! {
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let label = create_memo(move || format!("row {index}"));
                        view! {
                            <Row label={label} />
                        }
                    }}
                </ForEach>
            }}
            on_select={move |_: Vec<usize>| {}}
        >
            <Frame @node_ref={&region} width=REGION height=REGION />
        </ContextMenu>
    }
}

#[component]
fn Row(label: Memo<String>) -> MenuItem {
    let empty = create_memo(clone!(label -> move || label.with(String::is_empty)));
    view! {
        <MenuItem label={label} disabled={empty} />
    }
}

fn text_within(document: &Document, node: NodeId) -> Option<String> {
    if document.node_kind(node) == "text" {
        return Some(document.text(node).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| text_within(document, child))
}
