use super::*;
use crate::reactive::{NodeRef, Show, create_signal, view, with_reactive_scope};
use crate::styled::{Tabs, tabs_selected};

#[test]
fn a_show_adds_and_removes_a_tab_among_the_tabs_beside_it() {
    let tabs = NodeRef::new();
    let (extra, set_extra) = create_signal(false);
    let (document, [_tabs]) = toolbar_of({
        let tabs = tabs.clone();
        move || {
            [view! {
                <Tabs
                    @node_ref=&tabs
                    options={view! {
                        <unstyled::ChoiceOption label="List" />
                        <Show condition={extra}>
                            <unstyled::ChoiceOption label="Load" />
                        </Show>
                        <unstyled::ChoiceOption label="Name" />
                    }}
                    selected=0
                />
            }]
        }
    });
    let tabs = tabs.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        tab_labels(harness.document(), tabs),
        vec!["List".to_owned(), "Name".to_owned()],
        "a hidden show puts no tab between the tabs written around it"
    );

    with_reactive_scope(harness.document_mut(), move || set_extra.set(true));
    harness.frame(Vec::new());

    assert_eq!(
        tab_labels(harness.document(), tabs),
        vec!["List".to_owned(), "Load".to_owned(), "Name".to_owned()],
        "the tab a show builds keeps its place among the tabs written around it"
    );

    let third = harness.document().children(tabs)[2];
    harness.click(harness.center(third));
    harness.frame(Vec::new());
    assert_eq!(
        tabs_selected(harness.document(), tabs),
        2,
        "the tabs after a shown one report the index they now sit at"
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
    if document.node_kind(node) == "text" {
        return Some(document.text(node).to_owned());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| label_within(document, child))
}
