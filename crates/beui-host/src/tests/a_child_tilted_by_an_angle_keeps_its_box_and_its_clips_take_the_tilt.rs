use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui_core::display::flatten;
use beui_core::painter::Shape;

#[test]
fn a_child_tilted_by_an_angle_keeps_its_box_and_its_clips_take_the_tilt() {
    let _root = crate::reactive::RootScope::new();
    const MARK: Color32 = Color32::from_rgb(200, 30, 90);
    const TILT: f32 = 0.4;
    let turn = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0 align=Align::Start>
                <Frame width=200.0 height=200.0 padding={Sides::all(70.0)}>
                    <Frame @node_ref=&turn rotate=TILT>
                        <Canvas width=60.0 height=60.0>
                            <CanvasItem x=0.0 y=0.0 width=60.0 height=60.0>
                                <Frame fill=MARK radius=0.0 />
                            </CanvasItem>
                        </Canvas>
                    </Frame>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let tilted = harness.rect(turn.get());
    assert_eq!(tilted.size(), vec2(60.0, 60.0), "a tilt keeps the box");
    let (angle, bounds, clip) = flatten(&output.main().layers)
        .iter()
        .find_map(|shape| match shape {
            Shape::Rect {
                rect,
                color,
                rotation,
                clip,
                ..
            } if *color == MARK => Some((rotation.angle, rotation.bounds(*rect), *clip)),
            _ => None,
        })
        .expect("the child paints");
    assert!((angle - TILT).abs() < 1e-6, "tilted by the angle: {angle}");
    assert!(
        bounds.width() > 60.0 && clip.contains_rect(bounds),
        "a clip inside the tilt is tilted with it, so it cuts nothing off: {bounds:?} in {clip:?}"
    );
}
