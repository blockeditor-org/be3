use super::*;
use crate::reactive::{Button, Column, ForEach, KeyedStore, NodeRef, Text, build, view};

#[test]
fn editing_one_row_of_a_keyed_list_leaves_every_node_in_place() {
    let (list, edit) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (list, edit) = (list.clone(), edit.clone());
        move || {
            let store: KeyedStore<u32, String> = KeyedStore::new();
            store.reconcile_owned((0..3u32).map(|index| (index, format!("row {index}"))));
            let clicked = store.clone();
            let rows = store.clone();
            view! {
                <Column spacing=0.0>
                    <Button
                        @node_ref=&edit
                        on_click={move || {
                            clicked.reconcile_owned((0..3u32).map(|index| {
                                let label = if index == 1 { "edited" } else { "row" };
                                (index, format!("{label} {index}"))
                            }));
                        }}
                    >
                        <Text string="edit" />
                    </Button>
                    <Column @node_ref=&list spacing=0.0>
                        <ForEach keys={store.keys()}>
                            {move |index: u32| {
                                let item = rows.get(&index);
                                view! {
                                    <Text string={item} />
                                }
                            }}
                        </ForEach>
                    </Column>
                </Column>
            }
        }
    });

    let (list, edit) = (list.get(), edit.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let before = harness.document().children(list);
    assert_eq!(
        before
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["row 0", "row 1", "row 2"]
    );

    harness.click(harness.center(edit));
    harness.frame(Vec::new());

    let after = harness.document().children(list);
    assert_eq!(
        after, before,
        "editing an item must not rebuild any row node"
    );
    assert_eq!(
        after
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["row 0", "edited 1", "row 2"]
    );
}
