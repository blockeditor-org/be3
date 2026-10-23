use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_dynamic_child_can_fill_its_available_height() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        let state = create_signal(false);
        view! {
            <Frame>
                <List spacing=0.0>
                    <Dynamic value={state}>|_: bool|
                        <List @sizing=ItemSize::Percent(100.0) spacing=10.0>
                            <Frame height=20.0 />
                            <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                                <List spacing=0.0 />
                            </Offset>
                        </List>
                    </Dynamic>
                </List>
            </Frame>
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
