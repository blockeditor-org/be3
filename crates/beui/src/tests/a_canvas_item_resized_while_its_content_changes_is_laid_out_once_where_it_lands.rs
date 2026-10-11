use super::*;
use crate::reactive::{
    Layers, NodeRef, Overlay, OverlayAnchor, ReadSignal, clone, component, component_rect, create_memo,
    create_signal, with_reactive_scope,
};

#[test]
fn a_canvas_item_resized_while_its_content_changes_is_laid_out_once_where_it_lands() {
    let item = NodeRef::new();
    let (size, set_size) = create_signal(vec2(200.0, 100.0));
    let (inset, set_inset) = create_signal(0.0f32);
    let document = {
        let item = item.clone();
        build(move || {
            let width = create_memo(clone!(size -> move || size.get().x));
            let height = create_memo(move || size.get().y);
            view! {
                <Frame width=600.0 height=400.0>
                    <Canvas>
                        <CanvasItem x=10.0 y=10.0 width height>
                            <Anchored item={item.clone()} inset />
                        </CanvasItem>
                    </Canvas>
                </Frame>
            }
        })
    };
    let item = item.get();
    let mut harness = Harness::new(document);
    harness.settle();

    with_reactive_scope(harness.document_mut(), move || {
        set_size.set(vec2(300.0, 150.0));
        set_inset.set(4.0);
    });
    harness.settle();

    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(14.0, 10.0), vec2(292.0, 150.0)),
        "the content is laid out at the size the item was given"
    );
}

#[component]
fn Anchored(item: NodeRef, inset: ReadSignal<f32>) -> NodeId {
    let rect = component_rect();
    let anchor = create_memo(clone!(inset -> move || OverlayAnchor::Point(rect.get().max + vec2(inset.get(), 0.0))));
    view! {
        <Layers>
            <Frame padding_horizontal={inset}>
                <Frame @node_ref=&item />
            </Frame>
            <Overlay anchor traps_focus=false open=false>
                <Frame width=10.0 height=10.0 />
            </Overlay>
        </Layers>
    }
}
