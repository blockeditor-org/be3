use super::*;

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

fn drawing(requested: (u32, u32), made_for: Vec2) -> ViewDrawing {
    ViewDrawing::new(
        Blit {
            surface: 1,
            status: PresenterStatus::waiting(),
            shared: Rc::new(RefCell::new(Shared::default())),
            screen: ScreenId(1),
            quad: Quad::upright(Rect::ZERO),
            source: WHOLE,
            drawn: None,
            requested: Some(requested),
            placed: None,
            layer: None,
        },
        Some(made_for),
        Rc::new(RefCell::new(vec![Piece {
            local: WHOLE,
            source: WHOLE,
        }])),
        None,
        0.0,
    )
}

fn shown(drawing: &ViewDrawing, laid: Rect, painted: (u32, u32)) -> (Rect, Rect) {
    let blits = drawing.blits_in(laid);
    let (quad, source) = blits[0]
        .shown_at(painted)
        .expect("the frame shows in the view");
    (quad.rect, source)
}

#[test]
fn a_view_laid_out_at_a_size_its_frame_was_not_drawn_at_shows_that_frame_unscaled() {
    let at = pos2(10.0, 40.0);
    let made = drawing((300, 200), vec2(150.0, 100.0));

    assert_eq!(
        shown(
            &made,
            Rect::from_min_size(at, vec2(150.0, 100.0)),
            (300, 200)
        ),
        (Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE),
        "a frame drawn at the size the view is laid out at fills it"
    );
    assert_eq!(
        shown(
            &made,
            Rect::from_min_size(at, vec2(200.0, 100.0)),
            (300, 200)
        ),
        (Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE),
        "a view laid out wider than the drawing was made for, before its plugin draws the new size, shows its last frame at its own size from the top left rather than stretched"
    );
    assert_eq!(
        shown(
            &made,
            Rect::from_min_size(at, vec2(120.0, 100.0)),
            (300, 200)
        ),
        (
            Rect::from_min_size(at, vec2(120.0, 100.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(0.8, 1.0))
        ),
        "and laid out narrower, shows the left of it, cropped rather than squished"
    );

    let ahead = drawing((400, 200), vec2(200.0, 100.0));
    assert_eq!(
        shown(
            &ahead,
            Rect::from_min_size(at, vec2(150.0, 100.0)),
            (400, 200)
        ),
        (
            Rect::from_min_size(at, vec2(150.0, 100.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(0.75, 1.0))
        ),
        "a frame drawn for a size the host asked for before the view was laid out at it is cropped to the view, not squished into it"
    );
    assert_eq!(
        shown(
            &ahead,
            Rect::from_min_size(at, vec2(150.0, 100.0)),
            (300, 200)
        ),
        (Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE),
        "while the frame from before that request still fits the view as laid out"
    );
}
