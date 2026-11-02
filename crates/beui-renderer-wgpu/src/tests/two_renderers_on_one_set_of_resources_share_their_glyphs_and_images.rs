use super::*;

fn paint(painter: &Painter, image: &Image) {
    painter.text(
        pos2(4.0, 4.0),
        "beui",
        FontId::proportional(24.0),
        Color32::WHITE,
    );
    let rect = Rect::from_min_max(pos2(0.0, 48.0), pos2(16.0, 64.0));
    painter.image(
        rect,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        image,
        Color32::WHITE,
        0.0,
        false,
    );
}

#[test]
fn two_renderers_on_one_set_of_resources_share_their_glyphs_and_images() {
    let image = Image::from_rgba(1, 1, vec![255, 0, 0, 255]);
    let mut first = Target::new();
    let mut second = Target::sharing(&first);
    let resources = first.renderer.resources().clone();
    assert!(second.renderer.resources().same(&resources));

    first.draw(Color32::BLACK, Repaint::Everything, |painter| {
        paint(painter, &image)
    });
    second.draw(Color32::BLACK, Repaint::Everything, |painter| {
        paint(painter, &image)
    });

    for target in [&first, &second] {
        let capture = target.read();
        assert!(
            capture.brightest(Rect::from_min_max(Pos2::ZERO, pos2(64.0, 40.0))) > 200,
            "each renderer paints the text from the shared glyph atlas"
        );
        assert_eq!(
            capture.pixel(8, 56),
            [255, 0, 0, 255],
            "each paints the shared image"
        );
    }
    assert_eq!(
        resources.pictures(),
        1,
        "the image is uploaded once for both"
    );
    assert!(
        !resources.holds_colour_glyphs(),
        "no colour glyph was drawn, so the colour atlas is never made"
    );

    drop(first);
    assert_eq!(
        resources.pictures(),
        1,
        "the image stays while a renderer still shows it"
    );
    second.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.text(
            pos2(4.0, 4.0),
            "beui",
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    });
    assert_eq!(
        resources.pictures(),
        0,
        "the image goes once no renderer shows it"
    );
}
