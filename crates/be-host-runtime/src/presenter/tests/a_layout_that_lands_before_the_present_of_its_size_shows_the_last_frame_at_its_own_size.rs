use super::*;

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

    let layer = blit
        .screen_layer(&asked, Some((300, 200)), 0, 0)
        .expect("the texture on the surface shows");
    assert_eq!(
        (layer.surface, layer.bounds(), layer.clip),
        (
            Surface::Screen {
                instance: 1,
                surface: 4
            },
            last,
            Rect::from_min_size(Pos2::ZERO, vec2(300.0, 200.0))
        ),
        "natively the texture shows at its own size at the view's top left, not stretched to the \
         size the layout asked for"
    );
    let hole = blit
        .screen_layer(&asked, None, 0, 0)
        .expect("the web, which has no textures, still punches the hole for its canvas");
    assert_eq!(
        hole.bounds(),
        last,
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

    let style = crate::layers::canvas_style(&layer, 1.5, 3).expect("the canvas is placed");
    assert!(
        style.starts_with("transform:matrix(0.75,0,0,0.75,15,60);")
            && style.contains("clip-path:xywh(0px 0px 300px 200px ")
            && !style.contains("width:")
            && !style.contains("height:"),
        "the web keeps the canvas at its own size, three quarters of a CSS pixel to each of its \
         pixels at 1.5 CSS pixels a point and raster 2, from the view's corner: {style}"
    );
}
