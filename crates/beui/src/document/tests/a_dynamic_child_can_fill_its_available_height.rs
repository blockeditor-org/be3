use super::*;
use crate::reactive::{Column, Dynamic, Frame, NodeRef, Scroll, build, create_signal, view};

#[test]
fn a_dynamic_child_can_fill_its_available_height() {
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            let (state, _) = create_signal(false);
            view! {
                <Frame>
                    <Dynamic value={state}>
                        {move |_: bool| {
                            let scroll = scroll.clone();
                            view! {
                                <Column @sizing=ItemSize::Percent(100.0) spacing=10.0>
                                    <Frame height=20.0 />
                                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll />
                                </Column>
                            }
                        }}
                    </Dynamic>
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
