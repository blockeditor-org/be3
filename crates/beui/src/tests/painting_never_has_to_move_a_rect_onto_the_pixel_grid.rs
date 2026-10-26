use super::*;
use crate::draw::quads;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn painting_never_has_to_move_a_rect_onto_the_pixel_grid() {
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=3.0>
                <Frame @sizing=ItemSize::Percent(100.0) fill=Color32::WHITE padding=5.0>
                    <List spacing=0.0></List>
                </Frame>
                <Frame @sizing=ItemSize::Percent(200.0) fill=Color32::WHITE padding=5.0>
                    <List spacing=0.0></List>
                </Frame>
            </List>
        }
    });

    let mut harness = Harness::sized(document, Vec2::new(407.0, 200.0));
    harness.rescale(1.5);
    let output = harness.frame(Vec::new());

    let painted = quads(output.main(), output.pixels_per_point());
    let mut checked = 0;
    for quad in &painted.list {
        if let crate::draw::Quad::Rect { rect, .. } = quad {
            for edge in rect {
                assert_eq!(*edge, edge.round(), "a rect edge missed the pixel grid");
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "nothing was painted");
}
