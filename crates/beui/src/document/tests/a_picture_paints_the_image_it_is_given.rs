use super::*;
use crate::image::{Image, ImageFit};
use crate::reactive::{Frame, NodeRef, Picture, build, view};

#[test]
fn a_picture_paints_the_image_it_is_given() {
    let picture = NodeRef::new();
    let node = picture.clone();
    let image = Image::from_rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]);
    let painted = image.clone();
    let document = build(move || {
        view! {
            <Frame width=40.0 height=40.0>
                <Picture @node_ref={&node} image={Some(painted)} fit=ImageFit::Contain />
            </Frame>
        }
    });

    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let rect = harness.rect(picture.get());
    assert_eq!(rect.size(), Vec2::new(40.0, 40.0));
    let drawn: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Image { rect, image, .. } => Some((*rect, image.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1, "the picture paints its image once");
    assert_eq!(drawn[0].1, image);
    assert_eq!(
        drawn[0].0,
        Rect::from_min_size(pos2(0.0, 10.0), Vec2::new(40.0, 20.0)),
        "a contained image keeps its shape inside the box"
    );
}
