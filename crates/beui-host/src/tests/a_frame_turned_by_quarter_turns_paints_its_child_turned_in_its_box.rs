use super::*;
use crate::native::with_reactive_scope;
use crate::primitives::*;
use crate::reactive::*;
use beui_core::display::flatten;
use beui_core::painter::Shape;

#[test]
fn a_frame_turned_by_quarter_turns_paints_its_child_turned_in_its_box() {
    let _root = crate::reactive::RootScope::new();
    const MARK: Color32 = Color32::from_rgb(200, 30, 90);
    let turn = NodeRef::new();
    let quarters = create_signal(1u8);
    let document = build_node(move || {
        let angle = create_memo(move || f32::from(quarters.get()) * std::f32::consts::FRAC_PI_2);
        view! {
            <List spacing=0.0 align=Align::Start>
                <Frame @node_ref=&turn rotate={angle}>
                    <Frame width=40.0 height=40.0 fill=MARK radius=0.0 />
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let turned = harness.rect(turn.get());
    assert_eq!(turned.size(), vec2(40.0, 40.0), "a turn keeps the box");
    let painted = |output: &crate::native::FrameOutput| {
        flatten(&output.main().layers)
            .iter()
            .find_map(|shape| match shape {
                Shape::Rect {
                    rect,
                    color,
                    rotation,
                    ..
                } if *color == MARK => Some((rotation.bounds(*rect), rotation.angle)),
                _ => None,
            })
            .expect("the child paints")
    };
    let (bounds, angle) = painted(&output);
    assert!(
        (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
        "turned clockwise: {angle}"
    );
    assert!(
        (bounds.min - turned.min).length() < 0.01 && (bounds.max - turned.max).length() < 0.01,
        "the turned child covers the box it was turned into: {bounds:?} in {turned:?}"
    );

    with_reactive_scope(harness.document_mut(), move || quarters.set(2));
    let output = harness.frame(Vec::new());
    assert_eq!(harness.rect(turn.get()).size(), vec2(40.0, 40.0));
    let (bounds, angle) = painted(&output);
    assert!(
        (angle - std::f32::consts::PI).abs() < 1e-6,
        "turned half way: {angle}"
    );
    let upright = harness.rect(turn.get());
    assert!(
        (bounds.min - upright.min).length() < 0.01 && (bounds.max - upright.max).length() < 0.01,
        "a half turn keeps its box: {bounds:?} in {upright:?}"
    );
}
