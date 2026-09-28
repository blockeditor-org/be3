use super::*;
use crate::reactive::{Frame, ItemSize, List, view};

const MAX_WIDTH: f32 = 220.0;
const HEIGHT: f32 = 34.0;
const FILL: Color32 = Color32::from_rgb(1, 2, 3);

#[test]
fn a_frame_with_a_max_width_stops_growing_at_it_but_still_shrinks() {
    assert_eq!(painted_width(600.0), MAX_WIDTH);
    assert_eq!(painted_width(150.0), 150.0);
}

fn painted_width(container: f32) -> f32 {
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) width={container}>
                    <List spacing=0.0>
                        <Frame max_width=MAX_WIDTH height=HEIGHT color=FILL />
                    </List>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(700.0, 300.0));
    let output = harness.frame(Vec::new());
    output
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::Shape::Rect { rect, color, .. } if *color == FILL => Some(rect.width()),
            _ => None,
        })
        .expect("the frame painted its fill")
}
