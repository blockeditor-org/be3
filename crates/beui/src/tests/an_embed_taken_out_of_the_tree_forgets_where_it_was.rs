use super::*;
use crate::reactive::{Dynamic, Embed, EmbedSlot, Frame, create_signal, view, with_reactive_scope};

#[test]
fn an_embed_taken_out_of_the_tree_forgets_where_it_was() {
    let slot = EmbedSlot::new();
    let held = slot.clone();
    let (shown, set_shown) = create_signal(true);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={shown}>
                    {move |shown: bool| match shown {
                        true => view! {
                            <Embed slot={held.clone()} height=100.0 />
                        },
                        false => view! {
                            <Frame height=100.0 />
                        },
                    }}
                </Dynamic>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(slot.placement().is_some());

    with_reactive_scope(harness.document_mut(), move || set_shown.set(false));
    harness.frame(Vec::new());

    assert!(
        slot.placement().is_none(),
        "an embed that is gone no longer claims the place it was laid out at"
    );
}
