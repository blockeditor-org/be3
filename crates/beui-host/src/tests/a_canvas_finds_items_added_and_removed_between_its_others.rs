use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

fn spot(key: u32) -> (f32, f32) {
    match key {
        100 => (455.0, 275.0),
        _ => ((key % 10) as f32 * 90.0, (key / 10) as f32 * 60.0),
    }
}

#[test]
fn a_canvas_finds_items_added_and_removed_between_its_others() {
    let _root = crate::reactive::RootScope::new();
    let (canvas, change, last) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let keys = create_signal((0u32..40).collect::<Vec<_>>());
        let changed = move || {
            let mut next: Vec<u32> = (0..40).filter(|key| *key != 3).collect();
            next.insert(10, 100);
            keys.set(next);
        };
        view! {
            <List spacing=0.0>
                <Button @node_ref=&change on_click={changed}>
                    <Text>"change"</Text>
                </Button>
                <Canvas @sizing=ItemSize::Percent(100.0) @node_ref=&canvas>
                    <ForEach keys={keys}>|key: u32| {
                        let (x, y) = spot(key);
                        view! {
                            <CanvasItem x={x} y={y} width=20.0 height=10.0 />
                        }
                    }</ForEach>
                    <CanvasItem @node_ref=&last x=10.0 y=400.0 width=20.0 height=10.0 />
                </Canvas>
            </List>
        }
    });

    let (canvas, change, last) = (canvas.get(), change.get(), last.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(harness.document().children(canvas).len(), 41);

    harness.click(harness.center(change));
    harness.frame(Vec::new());

    let items = harness.document().children(canvas);
    assert_eq!(items.len(), 41);
    assert_eq!(
        harness
            .document()
            .node_rect(items[10])
            .map(|rect| rect.left()),
        Some(455.0),
        "an item added between the others is found where it was placed"
    );
    assert_eq!(items[40], last);
    assert!(
        harness.document().node_rect(last).is_some(),
        "an item after the ones that changed is still found"
    );
    for item in &items[..40] {
        assert!(
            harness.document().node_rect(*item).is_some(),
            "every item in view is laid out"
        );
    }
}
