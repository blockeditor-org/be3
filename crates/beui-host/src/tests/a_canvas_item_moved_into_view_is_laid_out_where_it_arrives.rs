use super::*;
use crate::reactive::*;

#[test]
fn a_canvas_item_moved_into_view_is_laid_out_where_it_arrives() {
    let _root = crate::reactive::RootScope::new();
    let item = NodeRef::new();
    let x = create_signal(VIEWPORT.x * 2.0);
    let document = {
        build_node(move || {
            view! {
                <Canvas>
                    <CanvasItem x={x} y=10.0 width=40.0 height=20.0>
                        <Frame @node_ref=&item />
                    </CanvasItem>
                </Canvas>
            }
        })
    };
    let item = item.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(
        harness.document().node_rect(item).is_none(),
        "a canvas lays out none of the items the view cannot see"
    );

    x.set(20.0);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(item).left(),
        20.0,
        "an item moved into view is laid out in the frame that moved it"
    );
}
