use super::*;
use crate::painter::Shape;
use crate::primitives::*;
use crate::reactive::*;

fn fill_clip(output: &FrameOutput) -> Rect {
    output
        .main()
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            Shape::Rect { color, clip, .. } if *color == Color32::from_rgb(200, 30, 30) => {
                Some(*clip)
            }
            _ => None,
        })
        .expect("the item's fill is painted")
}

#[test]
fn a_canvas_item_that_stops_clipping_moves_into_the_unclipped_space() {
    let _root = crate::reactive::RootScope::new();
    let clip = create_signal(true);
    let slot = EmbedSlot::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame width=100.0 height=100.0>
                    <Canvas>
                        <CanvasItem x=60.0 y=10.0 width=80.0 height=20.0 clip={clip}>
                            <Frame fill=Color32::from_rgb(200, 30, 30)>
                                <Embed slot={slot} />
                            </Frame>
                        </CanvasItem>
                    </Canvas>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let first = harness.frame(Vec::new());
    assert_eq!(
        slot.placement().expect("the embed was laid out").clip.max.x,
        100.0
    );
    assert_eq!(fill_clip(&first).max.x, 100.0);

    clip.set(false);
    let output = harness.frame(Vec::new());
    let laid_out = slot.placement().expect("the embed was laid out").clip;
    assert!(
        laid_out.max.x >= 140.0,
        "an item that stops clipping is laid out in the canvas's unclipped space, not {laid_out:?}"
    );
    let painted = fill_clip(&output);
    assert!(
        painted.max.x >= 140.0,
        "an item that stops clipping is painted in the canvas's unclipped space, not {painted:?}"
    );
}
