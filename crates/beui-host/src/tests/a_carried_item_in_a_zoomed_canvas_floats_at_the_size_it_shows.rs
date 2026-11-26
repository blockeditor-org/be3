use super::*;
use crate::drag::Carry;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_carried_item_in_a_zoomed_canvas_floats_at_the_size_it_shows() {
    let _root = RootScope::new();
    let red = Color32::from_rgb(200, 30, 30);
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <Canvas view={Some(CanvasView::new(Pos2::ZERO, 2.0))}>
                <CanvasItem x=10.0 y=10.0 width=50.0 height=20.0 scales=true>
                    <Interactive drag={Func::new(|_: PointerPress| Some(Carry::new(1_u32)))}>
                        <Frame @test_id="item" width=50.0 height=20.0 fill=red />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let home = harness.rect(harness.find("item"));
    assert_eq!(
        home.size(),
        vec2(100.0, 40.0),
        "the zoom draws the item twice its size"
    );
    let grab = home.center();
    let by = vec2(120.0, 100.0);
    harness.down(grab, PointerButton::Primary);
    for step in 1..=4 {
        harness.frame(vec![Input::PointerMoved(grab + by * (step as f32 / 4.0))]);
    }
    harness.settle();
    let drawn: Vec<Rect> = harness
        .frame(Vec::new())
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Rect { rect, color, .. } if *color == red => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn,
        vec![home.translate(by)],
        "carried, it floats at the size it shows at, not at its unzoomed size"
    );
    harness.up(grab + by, PointerButton::Primary);
    harness.settle();
}
