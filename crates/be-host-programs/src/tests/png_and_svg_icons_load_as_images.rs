use super::*;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8">
<rect width="16" height="8" fill="#ff0000"/></svg>"##;

#[test]
fn png_and_svg_icons_load_as_images() {
    let scratch = Scratch::new();
    let svg = scratch.write("icon.svg", SVG.as_bytes());
    let image = load_icon(&svg, 64).expect("the svg renders");
    assert_eq!((image.width(), image.height()), (64, 32));
    assert_eq!(&image.pixels()[..4], &[255, 0, 0, 255]);

    let mut png = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(&mut png),
        &[0, 255, 0, 128, 0, 0, 255, 255],
        2,
        1,
        image::ExtendedColorType::Rgba8,
    )
    .expect("the png encodes");
    let path = scratch.write("icon.png", &png);
    let image = load_icon(&path, 64).expect("the png decodes");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.pixels(), &[0, 255, 0, 128, 0, 0, 255, 255]);

    let broken = scratch.write("broken.png", b"not a png");
    assert!(load_icon(&broken, 64).is_none());
}
