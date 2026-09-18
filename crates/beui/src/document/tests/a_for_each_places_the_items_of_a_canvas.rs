use super::*;
use crate::reactive::{
    Button, Canvas, CanvasItem, Column, ForEach, ItemSize, Text, build, create_signal, view,
};

#[test]
fn a_for_each_places_the_items_of_a_canvas() {
    let (canvas, add) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (canvas, add) = (canvas.clone(), add.clone());
        move || {
            let (keys, set_keys) = create_signal(vec![0u32, 1]);
            view! {
                <Column spacing=0.0>
                    <Button @node_ref=&add on_click={move || set_keys.set(vec![0, 1, 2])}>
                        <Text string="add" />
                    </Button>
                    <Canvas @sizing=ItemSize::Percent(100.0) @node_ref=&canvas>
                        <ForEach keys={keys}>
                            {|index: u32| view! {
                                <CanvasItem x={index as f32 * 100.0} y=0.0 width=20.0 height=10.0 />
                            }}
                        </ForEach>
                    </Canvas>
                </Column>
            }
        }
    });

    let (canvas, add) = (canvas.get(), add.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let items = harness.document().children(canvas);
    assert_eq!(
        items.len(),
        2,
        "the items a `ForEach` builds are the canvas's own items"
    );
    assert_eq!(harness.rect(items[1]).left(), 100.0);

    harness.click(harness.center(add));
    harness.frame(Vec::new());

    let items = harness.document().children(canvas);
    assert_eq!(items.len(), 3);
    assert_eq!(
        harness.rect(items[2]).left(),
        200.0,
        "an item that arrives is placed where its own coordinates put it"
    );
}
