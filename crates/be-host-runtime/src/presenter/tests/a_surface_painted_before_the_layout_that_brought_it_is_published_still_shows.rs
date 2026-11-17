use super::*;

#[test]
fn a_surface_painted_before_the_layout_that_brought_it_is_published_still_shows() {
    let fresh = placement(300, 200);
    let shared = Rc::new(RefCell::new(Shared::default()));
    // The view's layout commit and its first present arrive in one pump: the present is heard
    // (on the web, as it lands) before the runtime publishes the layout that names the surface.
    shared.borrow_mut().paint(&[fresh]);
    shared.borrow_mut().publish(
        &ScreenLayout {
            screens: vec![fresh],
        },
        Vec::new(),
        None,
    );
    let blit = Blit {
        surface: 1,
        status: PresenterStatus::waiting(),
        shared: Rc::clone(&shared),
        screen: ScreenId(1),
        quad: Quad::upright(Rect::from_min_size(Pos2::ZERO, vec2(150.0, 100.0))),
        source: WHOLE,
        zoom: 1.0,
        picture: false,
        placed: None,
        layer: None,
    };
    let hole = blit
        .screen_layer(&fresh, None, 0)
        .expect("the web still punches the surface's hole once its layout is published");
    assert!(
        crate::layers::canvas_style(&hole, 1.0, 0).is_some(),
        "and places its canvas"
    );
    shared
        .borrow_mut()
        .publish(&ScreenLayout::default(), Vec::new(), None);
    assert!(
        blit.screen_layer(&fresh, None, 0).is_none(),
        "a surface the published layout no longer holds is forgotten"
    );
}
