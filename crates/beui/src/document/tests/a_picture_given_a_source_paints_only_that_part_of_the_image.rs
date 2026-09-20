use super::*;
use crate::image::{Image, ImageFit};
use crate::reactive::{Frame, NodeRef, Picture, build, view};

#[test]
fn a_picture_given_a_source_paints_only_that_part_of_the_image() {
    let picture = NodeRef::new();
    let node = picture.clone();
    let image = Image::from_rgba(4, 4, vec![128; 64]);
    let painted = image.clone();
    let source = Rect::from_min_size(pos2(0.5, 0.0), Vec2::new(0.5, 1.0));
    let document = build(move || {
        view! {
            <Frame width=40.0 height=40.0>
                <Picture
                    @node_ref={&node}
                    image={Some(painted)}
                    source={Some(source)}
                    fit=ImageFit::Contain
                />
            </Frame>
        }
    });

    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let drawn: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Image { rect, source, .. } => Some((*rect, *source)),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].1, source, "the quad samples only the named part");
    assert_eq!(
        drawn[0].0,
        Rect::from_min_size(pos2(10.0, 0.0), Vec2::new(20.0, 40.0)),
        "a half-width crop is laid out at the shape it will be drawn at"
    );
}
