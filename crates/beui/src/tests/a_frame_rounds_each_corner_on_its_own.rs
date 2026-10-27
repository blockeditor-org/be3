use super::*;
use crate::Corners;
use crate::primitives::*;
use crate::reactive::*;

const FILL: Color32 = Color32::from_rgb(9, 8, 7);

#[test]
fn a_frame_rounds_each_corner_on_its_own() {
    let document = build_node(|| {
        view! {
            <Frame width=100.0 height=40.0 fill=FILL radius={Corners::new(0.0, 8.0, 16.0, 8.0)} />
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 100.0));
    let output = harness.frame(Vec::new());
    let corners = output
        .main()
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::native::Shape::Rect {
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
