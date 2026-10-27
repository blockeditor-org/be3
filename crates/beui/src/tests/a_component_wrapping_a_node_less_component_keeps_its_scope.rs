use super::*;
use crate::native::Document;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::MenuItem;

const REGION: f32 = 60.0;

#[test]
fn a_component_wrapping_a_node_less_component_keeps_its_scope() {
    let _root = crate::reactive::RootScope::new();
    let menu = NodeRef::new();
    let region = NodeRef::new();
    let count = create_signal(0usize);
    let document = build_node(move || {
        view! {
            <Rows @node_ref=&menu region={region} count={count.read_only()} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let pos = harness.center(region.get());
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    count.set(2);
    harness.frame(Vec::new());

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu.get())
        .menu();
    assert_eq!(
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_count(),
        2,
        "a wrapper component contributes one item per row"
    );
    let labels: Vec<String> = (0..2)
        .map(|index| {
            let button = harness
                .document()
                .api::<unstyled::MenuListApi>(content)
                .row_button(index);
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
    let keys = create_memo(move || (0..count.get()).collect::<Vec<usize>>());
    view! {
        <ContextMenu
            items={view! {
                <ForEach keys={keys}>|index: usize| {
                    let label = create_memo(move || format!("row {index}"));
                    view! {
                        <Row label={label} />
                    }
                }</ForEach>
            }}
            on_select={move |_: Vec<usize>| {}}
        >
            <Frame @node_ref={&region} width=REGION height=REGION />
        </ContextMenu>
    }
}

#[component]
fn Row(label: Memo<String>) -> MenuItem {
    let empty = create_memo(move || label.with(String::is_empty));
    view! {
        <MenuItem label={label} disabled={empty} />
    }
}

fn text_within(document: &Document, node: NodeId) -> Option<String> {
    if let Some(text) = document.arena.kind_of::<TextNode>(node) {
        return Some(document.text(text).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| text_within(document, child))
}
