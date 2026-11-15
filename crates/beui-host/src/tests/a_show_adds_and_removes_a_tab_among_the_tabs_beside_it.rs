use super::*;
use crate::native::Document;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_show_adds_and_removes_a_tab_among_the_tabs_beside_it() {
    let _root = crate::reactive::RootScope::new();
    let tabs = NodeRef::new();
    let extra = create_signal(false);
    let (document, [_tabs]) = toolbar_of(move || {
        [view! {
            <Tabs
                @node_ref=&tabs
                options={view! {
                    <unstyled::ChoiceOption value="List" label="List" />
                    <Show condition={extra}>
                        <unstyled::ChoiceOption value="Load" label="Load" />
                    </Show>
                    <unstyled::ChoiceOption value="Name" label="Name" />
                }}
                selected="List"
            />
        }]
    });
    let tabs = tabs.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let list = shown_fit(harness.document(), tabs);

    assert_eq!(
        tab_labels(harness.document(), list),
        vec!["List".to_owned(), "Name".to_owned()],
        "a hidden show puts no tab between the tabs written around it"
    );

    extra.set(true);
    harness.frame(Vec::new());

    assert_eq!(
        tab_labels(harness.document(), list),
        vec!["List".to_owned(), "Load".to_owned(), "Name".to_owned()],
        "the tab a show builds keeps its place among the tabs written around it"
    );

    let third = harness.document().children(list)[2];
    harness.click(harness.center(third));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().api::<ChoiceApi<&str>>(tabs).selected(),
        Some("Name"),
        "a tab after a shown one still selects its own value"
    );
}

fn tab_labels(document: &Document, tabs: NodeId) -> Vec<String> {
    document
        .children(tabs)
        .into_iter()
        .filter_map(|tab| label_within(document, tab))
        .collect()
}

fn label_within(document: &Document, node: NodeId) -> Option<String> {
    if let Some(text) = document.arena.kind_of::<TextNode>(node) {
        return Some(document.text(text).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| label_within(document, child))
}
