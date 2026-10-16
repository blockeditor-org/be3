use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_embed_outliving_the_scope_that_listens_to_it_does_not_call_the_listener() {
    let _root = crate::reactive::RootScope::new();
    let (shown, set_shown) = create_signal(true);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={shown}>
                    {move |shown: bool| match shown {
                        true => {
                            let owned = store(());
                            let slot = EmbedSlot::new();
                            slot.on_place(move |_| owned.with(|_| ()));
                            view! {
                                <Frame height=100.0>
                                    <Embed slot={slot} />
                                </Frame>
                            }
                        }
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

    set_shown.set(false);
    harness.frame(Vec::new());
}
