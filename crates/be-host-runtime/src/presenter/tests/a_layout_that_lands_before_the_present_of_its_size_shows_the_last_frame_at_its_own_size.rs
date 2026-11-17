use super::*;

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

fn placement(width: u32, height: u32) -> ScreenPlacement {
    ScreenPlacement {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        surface: 4,
        width,
        height,
        scale_factor_millis: 2000,
        raster_millis: 2000,
    }
}

#[test]
fn a_layout_that_lands_before_the_present_of_its_size_shows_the_last_frame_at_its_own_size() {
    // The window grew: the layout already asks for 400x260 pixels (200x130 points at raster
    // 2), but the last present, and so the texture, the worker's canvas and the painting, is
    // still 300x200.
    let asked = placement(400, 260);
    let shared = Rc::new(RefCell::new(Shared::default()));
    shared.borrow_mut().layout = ScreenLayout {
        screens: vec![asked],
    };
    shared.borrow_mut().paint(&[placement(300, 200)]);
    shared.borrow_mut().paintings.insert(
        ScreenId(1),
        Painting {
            frame: beui::native::painting::Frame {
                size: [300, 200],
                pixels_per_point: 2.0,
                background: [10, 20, 30, 255],
                primitives: Vec::new(),
            },
            textures: Default::default(),
        },
    );
    let laid = Rect::from_min_size(pos2(10.0, 40.0), vec2(200.0, 130.0));
    let last = Rect::from_min_size(pos2(10.0, 40.0), vec2(150.0, 100.0));
    let drawing = ViewDrawing::new(
        Blit {
            surface: 1,
            status: PresenterStatus::waiting(),
            shared: Rc::clone(&shared),
            screen: ScreenId(1),
            quad: Quad::upright(Rect::ZERO),
            source: WHOLE,
            zoom: 1.0,
            picture: false,
            placed: None,
            layer: None,
        },
        Rc::new(RefCell::new(vec![Piece {
            local: WHOLE,
            source: WHOLE,
        }])),
        None,
        0.0,
    );
    let blit = drawing.blits_in(laid).remove(0);

    let (surface, quad, source) = blit
        .on_surface(&asked, Some((300, 200)))
        .expect("the texture on the surface shows");
    assert_eq!(
        (surface, quad.rect, source),
        (4, last, WHOLE),
        "natively the texture shows at its own size at the view's top left, not stretched to the \
         size the layout asked for"
    );
    let (_, hole, _) = blit
        .on_surface(&asked, None)
        .expect("the web, which has no textures, still punches the hole for its canvas");
    assert_eq!(
        hole.rect, last,
        "the hole the web punches is the size the worker last painted, where its canvas shows"
    );

    let painting = drawing.painting(laid).expect("the view paints");
    let shown: Vec<[f32; 4]> = painting
        .frame
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.content {
            beui::native::painting::Content::RoundedRect(rect) => Some(rect.rect),
            _ => None,
        })
        .collect();
    assert_eq!(
        shown,
        vec![[last.min.x, last.min.y, last.max.x, last.max.y]],
        "a paint snapshot shows the painting at its own size at the view's top left"
    );

    let style = canvas_style(quad, source, (300, 200), 1.5, 3).expect("the canvas is placed");
    assert!(
        style.starts_with("transform:matrix(0.75,0,0,0.75,15,60);")
            && style.contains("clip-path:xywh(0px 0px 300px 200px ")
            && !style.contains("width:")
            && !style.contains("height:"),
        "the web keeps the canvas at its own size, three quarters of a CSS pixel to each of its \
         pixels at 1.5 CSS pixels a point and raster 2, from the view's corner: {style}"
    );
}

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
    let (_, hole, source) = blit
        .on_surface(&fresh, None)
        .expect("the web still punches the surface's hole once its layout is published");
    assert!(
        canvas_style(hole, source, (300, 200), 1.0, 0).is_some(),
        "and places its canvas"
    );
    shared.borrow_mut().publish(&ScreenLayout::default(), Vec::new(), None);
    assert!(
        blit.on_surface(&fresh, None).is_none(),
        "a surface the published layout no longer holds is forgotten"
    );
}
