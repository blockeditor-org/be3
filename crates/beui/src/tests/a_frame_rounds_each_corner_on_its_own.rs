use super::*;
use crate::Corners;
use crate::reactive::{Frame, view};

const FILL: Color32 = Color32::from_rgb(9, 8, 7);

#[test]
fn a_frame_rounds_each_corner_on_its_own() {
    let document = build(|| {
        view! {
            <Frame
                width=100.0
                height=40.0
                color=FILL
                radius=8
                radius_top_left=0
                radius_bottom_right=16
            />
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 100.0));
    let output = harness.frame(Vec::new());
    let corners = output
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::Shape::Rect {
                color,
                corner_radius,
                ..
            } if *color == FILL => Some(*corner_radius),
            _ => None,
        })
        .expect("the frame painted its fill");
    assert_eq!(
        corners,
        Corners {
            top_left: 0.0,
            top_right: 8.0,
            bottom_right: 16.0,
            bottom_left: 8.0,
        }
    );
}
