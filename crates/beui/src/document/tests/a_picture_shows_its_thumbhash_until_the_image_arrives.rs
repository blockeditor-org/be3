use super::*;
use crate::image::{Image, ImageFit};
use crate::reactive::{Frame, Picture, build, create_signal, view, with_reactive_scope};

fn painted(output: &crate::FrameOutput) -> Vec<(Rect, Image, bool)> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Image {
                rect,
                image,
                smooth,
                ..
            } => Some((*rect, image.clone(), *smooth)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_picture_shows_its_thumbhash_until_the_image_arrives() {
    let pixels: Vec<u8> = (0..30 * 7)
        .flat_map(|at: u32| [(at % 256) as u8, 90, 200 - (at % 200) as u8, 255])
        .collect();
    let image = Image::from_rgba(30, 7, pixels);
    let thumbhash = image.thumbhash();
    assert_eq!((thumbhash.width, thumbhash.height), (30, 7));
    let placeholder = thumbhash.decode().expect("a fresh thumbhash decodes");

    let (shown, set_shown) = create_signal(None::<Image>);
    let document = build(move || {
        view! {
            <Frame width=40.0 height=40.0>
                <Picture
                    image={shown}
                    thumbhash={Some(thumbhash)}
                    fit=ImageFit::Contain
                    smooth=false
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let placed = ImageFit::Contain.place(
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(40.0, 40.0)),
        Vec2::new(30.0, 7.0),
    );

    let before = painted(&harness.frame(Vec::new()));
    assert_eq!(
        before.len(),
        1,
        "the placeholder is painted while there is no image"
    );
    assert_eq!(
        (before[0].1.width(), before[0].1.height()),
        (placeholder.width(), placeholder.height())
    );
    assert_eq!(
        before[0].0, placed,
        "the placeholder takes the exact shape of the image it stands for"
    );
    assert!(
        before[0].2,
        "a thumbhash is always smoothed, since it is a blur"
    );

    let arrived = image.clone();
    with_reactive_scope(harness.document_mut(), move || set_shown.set(Some(arrived)));
    let after = painted(&harness.frame(Vec::new()));
    assert_eq!(after.len(), 1, "the image replaces the placeholder");
    assert_eq!(after[0].1, image);
    assert_eq!(
        after[0].0, placed,
        "the image lands where its placeholder was"
    );
    assert!(!after[0].2, "the image keeps its own sampling");
}
