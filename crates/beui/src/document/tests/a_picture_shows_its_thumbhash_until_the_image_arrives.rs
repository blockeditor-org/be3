use super::*;
use crate::image::{Image, ImageFit};
use crate::reactive::{Frame, Picture, build, create_signal, view, with_reactive_scope};

fn painted(output: &crate::FrameOutput) -> Vec<(Image, bool)> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Image { image, smooth, .. } => Some((image.clone(), *smooth)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_picture_shows_its_thumbhash_until_the_image_arrives() {
    let pixels: Vec<u8> = (0..16 * 8)
        .flat_map(|at: u32| [(at * 2) as u8, 90, 200 - at as u8, 255])
        .collect();
    let image = Image::from_rgba(16, 8, pixels);
    let hash = image.thumbhash();
    let placeholder = Image::from_thumbhash(&hash).expect("a fresh thumbhash decodes");
    assert!(
        placeholder.width() > placeholder.height(),
        "the placeholder keeps the image's landscape shape"
    );

    let (shown, set_shown) = create_signal(None::<Image>);
    let document = build(move || {
        view! {
            <Frame width=40.0 height=40.0>
                <Picture image={shown} thumbhash={Some(hash)} fit=ImageFit::Contain smooth=false />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);

    let before = painted(&harness.frame(Vec::new()));
    assert_eq!(
        before.len(),
        1,
        "the placeholder is painted while there is no image"
    );
    assert_eq!(
        (before[0].0.width(), before[0].0.height()),
        (placeholder.width(), placeholder.height())
    );
    assert!(
        before[0].1,
        "a thumbhash is always smoothed, since it is a blur"
    );

    let arrived = image.clone();
    with_reactive_scope(harness.document_mut(), move || set_shown.set(Some(arrived)));
    let after = painted(&harness.frame(Vec::new()));
    assert_eq!(after.len(), 1, "the image replaces the placeholder");
    assert_eq!(after[0].0, image);
    assert!(!after[0].1, "the image keeps its own sampling");
}
