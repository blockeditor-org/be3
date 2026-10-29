use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Container, Scroll};

#[test]
fn a_scroll_keeps_its_place_when_a_container_above_reflows_after_a_resize() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let rows: Vec<NodeRef> = (0..40).map(|_| NodeRef::new()).collect();
    let document = build_node({
        let rows = rows.clone();
        move || {
            view! {
                <Scroll @node_ref=&scroll>
                    <List spacing=0.0>
                        <Text>
                            "A paragraph above the rows wraps onto more lines as the scroll narrows, \
                             so the rows under it move down in the same layout that hears the new \
                             width, before the container below it has heard it."
                        </Text>
                        <Container>|size: ReadSignal<Vec2>| {
                            let height = create_memo(move || match size.get().x < 300.0 {
                                true => 300.0,
                                false => 100.0,
                            });
                            view! {
                                <Frame height fill=Color32::WHITE />
                            }
                        }</Container>
                        <ForEach keys={indices(rows.len())}>|index: usize|
                            <Frame @node_ref={&rows[index]} height=40.0 />
                        </ForEach>
                    </List>
                </Scroll>
            }
        }
    });
    let scroll = scroll.get();
    let row = rows[20].get();
    let mut harness = Harness::sized(document, vec2(400.0, 300.0));
    harness.frame(Vec::new());
    harness.document_mut().set_scroll_offset(scroll, 700.0);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let top = harness.rect(row).top();

    harness.resize(vec2(250.0, 300.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(row).top(),
        top,
        "the row the scroll was resting on stays put while what is above it reflows"
    );
}
