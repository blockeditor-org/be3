use super::*;
use crate::image::{Image, ImageFit};
use crate::primitives::*;
use crate::unstyled::Picture;

#[test]
fn a_covering_picture_paints_only_the_part_inside_its_frame() {
    let image = Image::from_rgba(20, 10, vec![128; 800]);
    let document = build_node(move || {
        view! {
            <Frame width=10.0 height=10.0>
                <Picture image={Some(image)} fit=ImageFit::Cover radius=5.0 />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let painted = output
        .main()
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::painter::Shape::Image { rect, source, .. } => Some((*rect, *source)),
            _ => None,
        })
        .expect("the picture is painted");
    assert_eq!(
        painted,
        (
            Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(10.0, 10.0)),
            Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0)),
        ),
        "the overflowing sides are cut from the source, so the rounded corners land on the frame's"
    );
}
