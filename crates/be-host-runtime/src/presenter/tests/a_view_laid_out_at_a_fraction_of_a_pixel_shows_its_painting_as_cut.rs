use super::*;

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

#[test]
fn a_view_laid_out_at_a_fraction_of_a_pixel_shows_its_painting_as_cut() {
    let quad = Quad::upright(Rect::from_min_size(pos2(4.0, 6.0), vec2(100.3, 50.0)));
    for width in [100, 101] {
        let (shown, source) =
            unscaled(quad, WHOLE, (width, 50), 1.0).expect("the painting shows");
        assert_eq!(
            (shown.rect, source),
            (quad.rect, WHOLE),
            "a view 100.3 points wide at raster 1 is painted at {width} pixels, its size \
             rounded: that painting is the view's own and shows as the piece was cut"
        );
    }
    let (shown, source) = unscaled(quad, WHOLE, (99, 50), 1.0).expect("the painting shows");
    assert_eq!(
        (shown.rect, source),
        (Rect::from_min_size(pos2(4.0, 6.0), vec2(99.0, 50.0)), WHOLE),
        "a painting a pixel and more narrower is an older one, shown at its own size"
    );
}
