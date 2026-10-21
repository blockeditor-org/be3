use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_embed_replaced_by_another_on_its_slot_leaves_the_new_one_placed() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let tall = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={tall}>|tall: bool| {
                    let height = match tall {
                        true => 200.0,
                        false => 100.0,
                    };
                    let slot = slot;
                    view! {
                        <Frame height=height>
                            <Embed slot={slot} />
                        </Frame>
                    }
                }</Dynamic>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    tall.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        slot.placement().map(|placement| placement.rect.height()),
        Some(200.0),
        "the embed now on the slot keeps its place when the old one goes"
    );
}
