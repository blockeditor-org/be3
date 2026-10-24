use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui_core::display::flatten;
use beui_core::painter::Shape;

#[test]
fn a_change_inside_a_turned_child_still_paints_turned() {
    let _root = crate::reactive::RootScope::new();
    const MARK: Color32 = Color32::from_rgb(200, 30, 90);
    let width = create_signal(10.0f32);
    let document = build_node(move || {
        view! {
            <List spacing=0.0 align=Align::Start>
                <Turn quarter_turns=1u8>
                    <Frame width=60.0 height=60.0 radius=0.0>
                        <Frame
                            width={create_memo(move || Extent::px(width.get()))}
                            height=20.0
                            fill=MARK
                            radius=0.0
                        />
                    </Frame>
                </Turn>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || width.set(30.0));
    let output = harness.frame(Vec::new());

    let angle = flatten(&output.main().layers)
        .iter()
        .find_map(|shape| match shape {
            Shape::Rect {
                color, rotation, ..
            } if *color == MARK => Some(rotation.angle),
            _ => None,
        })
        .expect("the changed child paints");
    assert!(
        (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
        "a child repainted on its own keeps the turn it is held in: {angle}"
    );
}
