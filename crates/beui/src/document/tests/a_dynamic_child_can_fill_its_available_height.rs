use super::*;
use crate::reactive::{Dynamic, Frame, List, NodeRef, Offset, build, create_signal, view};

#[test]
fn a_dynamic_child_can_fill_its_available_height() {
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            let (state, _) = create_signal(false);
            view! {
                <Frame>
                    <List spacing=0.0>
                        <Dynamic value={state}>
                            {move |_: bool| {
                                let scroll = scroll.clone();
                                view! {
                                    <List @sizing=ItemSize::Percent(100.0) spacing=10.0>
                                        <Frame height=20.0 />
                                        <Offset
                                            @sizing=ItemSize::Percent(100.0)
                                            @node_ref=&scroll
                                        />
                                    </List>
                                }
                            }}
                        </Dynamic>
                    </List>
                </Frame>
            }
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());

    assert_eq!(
        harness.document().node_rect(scroll).unwrap().height(),
        270.0
    );
}
