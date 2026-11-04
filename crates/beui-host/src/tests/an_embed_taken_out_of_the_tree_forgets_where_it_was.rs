use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_embed_taken_out_of_the_tree_forgets_where_it_was() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let shown = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={shown}>|shown: bool| {
                    match shown {
                        true => {
                            let slot = slot;
                            view! {
                                <Frame height=100.0>
                                    <Embed slot={slot} />
                                </Frame>
                            }
                        }
                        false => view! {
                            <Frame height=100.0 />
                        },
                    }
                }</Dynamic>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(slot.placement().is_some());

    shown.set(false);
    harness.frame(Vec::new());

    assert!(
        slot.placement().is_none(),
        "an embed that is gone no longer claims the place it was laid out at"
    );
}
