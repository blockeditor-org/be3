use super::*;
use crate::reactive::{Frame, Dynamic, Embed, EmbedSlot, create_signal, view, with_reactive_scope};

#[test]
fn an_embed_replaced_by_another_on_its_slot_leaves_the_new_one_placed() {
    let slot = EmbedSlot::new();
    let held = slot.clone();
    let (tall, set_tall) = create_signal(false);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={tall}>
                    {move |tall: bool| {
                        let height = match tall {
                            true => 200.0,
                            false => 100.0,
                        };
                        let slot = held.clone();
                        view! {
                            <Frame height=height><Embed slot={slot} /></Frame>
                        }
                    }}
                </Dynamic>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_tall.set(true));
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        slot.placement().map(|placement| placement.rect.height()),
        Some(200.0),
        "the embed now on the slot keeps its place when the old one goes"
    );
}
