use super::*;

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

fn placement(width: u32, height: u32, raster: f32) -> ScreenPlacement {
    ScreenPlacement {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        surface: 1,
        width,
        height,
        scale_factor_millis: (raster * 1000.0) as u32,
        raster_millis: (raster * 1000.0) as u32,
    }
}

fn drawing(pieces: Vec<Piece>, zoom: f32, rotation: f32) -> ViewDrawing {
    ViewDrawing::new(
        Blit {
            surface: 1,
            status: PresenterStatus::waiting(),
            shared: Rc::new(RefCell::new(Shared::default())),
            screen: ScreenId(1),
            quad: Quad::upright(Rect::ZERO),
            source: WHOLE,
            zoom,
            placed: None,
            layer: None,
        },
        Rc::new(RefCell::new(pieces)),
        None,
        rotation,
    )
}

fn shown(laid: Rect, painted: ScreenPlacement) -> Option<(Rect, Rect)> {
    let blits = drawing(vec![Piece { local: WHOLE, source: WHOLE }], 1.0, 0.0).blits_in(laid);
    blits[0]
        .shown_at(&painted)
        .map(|(quad, source)| (quad.rect, source))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn an_apps_painting_is_shown_at_its_own_size_whatever_size_its_view_is_laid_out_at() {
    let at = pos2(10.0, 40.0);
    assert_eq!(
        shown(Rect::from_min_size(at, vec2(150.0, 100.0)), placement(300, 200, 2.0)),
        Some((Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE)),
        "a painting drawn at the size the view is laid out at fills it"
    );
    assert_eq!(
        shown(Rect::from_min_size(at, vec2(200.0, 130.0)), placement(300, 200, 2.0)),
        Some((Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE)),
        "a view laid out larger before its app draws the new size shows the last painting at its \
         own size from the top left, leaving the rest to the backdrop, rather than stretching it"
    );
    assert_eq!(
        shown(Rect::from_min_size(at, vec2(120.0, 100.0)), placement(300, 200, 2.0)),
        Some((
            Rect::from_min_size(at, vec2(120.0, 100.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(0.8, 1.0))
        )),
        "and laid out narrower, shows the left of it, cropped rather than squished"
    );
    assert_eq!(
        shown(Rect::from_min_size(at, vec2(150.0, 100.0)), placement(150, 100, 1.0)),
        Some((Rect::from_min_size(at, vec2(150.0, 100.0)), WHOLE)),
        "a painting at another raster covers the same points: its pixels over its raster"
    );

    // The invariant, over view sizes, pieces, paintings, rasters, zooms and turns: one point of
    // what shows is always the painting's pixels per point along both axes, and what shows starts
    // at the textured part's top left and never leaves the piece.
    let halves = [
        vec![Piece { local: WHOLE, source: WHOLE }],
        vec![
            Piece {
                local: Rect::from_min_max(Pos2::ZERO, pos2(0.5, 1.0)),
                source: Rect::from_min_max(Pos2::ZERO, pos2(0.5, 1.0)),
            },
            Piece {
                local: Rect::from_min_max(pos2(0.5, 0.25), pos2(1.0, 1.0)),
                source: Rect::from_min_max(pos2(0.5, 0.25), pos2(1.0, 1.0)),
            },
        ],
        // A view whose top is scrolled out of sight: its texture covers only the rest.
        vec![Piece {
            local: Rect::from_min_max(pos2(0.0, 0.4), pos2(1.0, 1.0)),
            source: WHOLE,
        }],
    ];
    for pieces in &halves {
        for laid in [vec2(150.0, 100.0), vec2(311.5, 87.25), vec2(40.0, 400.0)] {
            for (width, height) in [(300, 200), (1, 1), (640, 90), (75, 512)] {
                for raster in [1.0, 1.25, 2.0, 3.0] {
                    for zoom in [1.0, 0.5, 2.5] {
                        for rotation in [0.0, 0.7] {
                            let painted = placement(width, height, raster);
                            let density = raster / zoom;
                            let rect = Rect::from_min_size(at, laid);
                            let drawing = drawing(pieces.clone(), zoom, rotation);
                            for (blit, piece) in drawing.blits_in(rect).iter().zip(pieces) {
                                let Some((quad, source)) = blit.shown_at(&painted) else {
                                    continue;
                                };
                                let case = format!(
                                    "{piece:?} of a view {laid:?} showing {width}x{height} at \
                                     raster {raster}, zoom {zoom}, turned {rotation}"
                                );
                                assert!(
                                    close(quad.rect.width() * density, source.width() * width as f32)
                                        && close(
                                            quad.rect.height() * density,
                                            source.height() * height as f32
                                        ),
                                    "{case}: shown at {:?} from {source:?}, scaled",
                                    quad.rect
                                );
                                assert!(
                                    source.min.x >= -1e-4
                                        && source.min.y >= -1e-4
                                        && source.max.x <= 1.0 + 1e-4
                                        && source.max.y <= 1.0 + 1e-4,
                                    "{case}: reads {source:?}, past the painting"
                                );
                                let whole = blit.quad.rect;
                                assert!(
                                    quad.rect.width() <= whole.width() + 1e-3
                                        && quad.rect.height() <= whole.height() + 1e-3,
                                    "{case}: shows {:?}, past its piece {whole:?}",
                                    quad.rect
                                );
                                if rotation == 0.0 {
                                    let textured = vec2(
                                        whole.width() / piece.source.width(),
                                        whole.height() / piece.source.height(),
                                    );
                                    let origin = whole.min
                                        - vec2(
                                            piece.source.min.x * textured.x,
                                            piece.source.min.y * textured.y,
                                        );
                                    let from = origin
                                        + vec2(
                                            source.min.x * width as f32,
                                            source.min.y * height as f32,
                                        ) / density;
                                    assert!(
                                        close(quad.rect.min.x, from.x)
                                            && close(quad.rect.min.y, from.y),
                                        "{case}: shows from {:?}, not anchored at the textured \
                                         part's top left",
                                        quad.rect.min
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn an_anchored_view_reaching_past_its_painting_shows_what_it_has_at_its_own_size() {
    let slot = Quad::upright(Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 50.0)));
    let beyond = Rect::from_min_max(Pos2::ZERO, pos2(1.25, 1.0));
    let (layer, source) = within_texture(slot, beyond)
        .expect("an anchored view whose pixels reach past its painting shows what it has");
    assert_eq!(
        (layer.rect, source),
        (Rect::from_min_size(pos2(10.0, 20.0), vec2(80.0, 50.0)), WHOLE),
        "an anchored view that grew before its app drew the new size shows its last frame at its own size"
    );
}
