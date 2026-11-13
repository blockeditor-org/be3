use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_scroll_in_a_zoomed_canvas_in_the_update_that_arms_a_glide_moves_no_row() {
    let _root = RootScope::new();
    let order = create_signal(vec![1_u64, 2, 3, 4]);
    let arm = create_signal(0_u64);
    let scrolled = create_signal(0.0_f32);
    let document = build_node(move || {
        let view = create_memo(move || Some(CanvasView::new(Pos2::ZERO, 2.0)));
        view! {
            <Canvas view>
                <CanvasItem x=0.0 y=0.0 width=100.0 height={2.0 * GLIDE_ROW} scales=true>
                    <Offset offset={scrolled}>
                        <List spacing=0.0>
                            <ForEach keys={order}>|key: u64|
                                <unstyled::Glide @test_id={format!("slot.{key}")} arm={arm}>
                                    <Frame @test_id={format!("row.{key}")} height=GLIDE_ROW />
                                </unstyled::Glide>
                            </ForEach>
                        </List>
                    </Offset>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(top_of(&harness, "row.4"), 2.0 * 3.0 * GLIDE_ROW);

    order.set(vec![2, 1, 3, 4]);
    arm.update(|arm| *arm += 1);
    scrolled.set(GLIDE_ROW);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.4"),
        2.0 * 2.0 * GLIDE_ROW,
        "a row the reorder left in its place is drawn where the zoomed scroll put it, with no \
         glide left over from the zoom"
    );
}
