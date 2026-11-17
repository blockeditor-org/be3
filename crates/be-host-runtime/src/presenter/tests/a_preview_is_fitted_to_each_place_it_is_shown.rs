use super::*;

#[test]
fn a_preview_is_fitted_to_each_place_it_is_shown() {
    let shared = Rc::new(RefCell::new(Shared::default()));
    let small = Rect::from_min_size(pos2(5.0, 5.0), vec2(60.0, 40.0));
    let blit = Blit {
        surface: 1,
        status: PresenterStatus::waiting(),
        shared,
        screen: ScreenId(1),
        quad: Quad::upright(small),
        source: WHOLE,
        zoom: 1.0,
        picture: true,
        placed: None,
        layer: None,
    };
    let (quad, source) = blit
        .shown_at(Painted {
            size: (300, 200),
            raster: 1.0,
        })
        .expect("the preview shows");
    assert_eq!(
        (quad.rect, source),
        (small, WHOLE),
        "a preview painted for a larger place is a picture: the whole of it fits a smaller one"
    );
}
