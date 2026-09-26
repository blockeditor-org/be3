use super::*;
use crate::image::{Image, ImageFit};
use crate::reactive::{Frame, Picture, build, view};

fn drawn(width: f32, height: f32) -> Rect {
    let image = Image::from_rgba(20, 10, vec![128; 800]);
    let document = build(move || {
        view! {
            <Frame width={width} height={height}>
                <Picture image={Some(image)} fit=ImageFit::ScaleDown />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    output
        .shapes
        .iter()
        .find_map(|shape| match shape {
            crate::painter::Shape::Image { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("the picture is painted")
}

#[test]
fn a_picture_scaled_down_never_grows_past_its_own_pixels() {
    assert_eq!(
        drawn(100.0, 100.0),
        Rect::from_min_size(pos2(40.0, 45.0), Vec2::new(20.0, 10.0)),
        "a small image stays at its own size, centred"
    );
    assert_eq!(
        drawn(10.0, 40.0),
        Rect::from_min_size(pos2(0.0, 17.5), Vec2::new(10.0, 5.0)),
        "a large image shrinks to fit"
    );
}
