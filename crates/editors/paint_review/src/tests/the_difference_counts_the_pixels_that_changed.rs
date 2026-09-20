use block_editor_plugin::beui::Image;

#[test]
fn the_difference_counts_the_pixels_that_changed() {
    let approved = filled(4, 4);
    let mut pixels = approved.pixels().to_vec();
    for index in [5, 6] {
        pixels[index * 4] = 255;
    }
    let current = Image::from_rgba(4, 4, pixels);

    let painted = crate::render::difference(&approved, &current);
    assert_eq!((painted.image.width(), painted.image.height()), (4, 4));
    assert_eq!(
        painted.description,
        "2 pixels differ, in a 2x1 region at (1, 1)"
    );

    let same = crate::render::difference(&approved, &approved);
    assert_eq!(
        same.description,
        "these frames are the same, pixel for pixel"
    );

    let taller = filled(4, 6);
    let grown = crate::render::difference(&approved, &taller);
    assert_eq!((grown.image.width(), grown.image.height()), (4, 6));
    assert!(
        grown
            .description
            .starts_with("the painting is 4x6, it used to be 4x4; ")
    );
}

fn filled(width: u32, height: u32) -> Image {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for alpha in pixels.iter_mut().skip(3).step_by(4) {
        *alpha = 255;
    }
    Image::from_rgba(width, height, pixels)
}
