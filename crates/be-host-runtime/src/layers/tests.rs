use std::{cell::RefCell, rc::Rc};

use be_sdk_api::{EditorInstanceId, ScreenId, ScreenLayout, ScreenPlacement, ViewId};
use beui::{Pos2, vec2};

use super::*;
use crate::presenter::{AnchoredSource, Blit, PresenterStatus, Shared};

const WHOLE: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));

fn at() -> DrawAt {
    DrawAt {
        rect: [0.0, 0.0, 800.0, 600.0],
        clip: [10.0, 20.0, 700.0, 500.0],
        screen: vec2(800.0, 600.0),
        pixels_per_point: 2.0,
        format: wgpu::TextureFormat::Bgra8Unorm,
        renderer: 0,
    }
}

fn close(one: [f32; 4], other: [f32; 4]) -> bool {
    one.iter().zip(other).all(|(one, other)| (one - other).abs() < 1e-3)
}

#[test]
fn a_layer_draws_natively_as_the_quad_its_blit_drew() {
    let quads = [
        Quad::upright(Rect::from_min_size(pos2(10.0, 40.0), vec2(150.0, 100.0))),
        Quad {
            rect: Rect::from_min_size(pos2(30.0, 50.0), vec2(120.0, 80.0)),
            rotation: 0.4,
            opacity: 0.5,
            corner_radius: 6.0,
        },
    ];
    let source = Rect::from_min_max(pos2(0.25, 0.0), pos2(1.0, 0.5));
    for quad in quads {
        let layer = screen(
            LayerId(1),
            Surface::Root(0),
            (quad, source),
            Painted {
                size: (300, 200),
                raster: 2.0,
            },
        )
        .expect("the blit shows");
        let drawn = texture_quad(&layer, &at());
        let was = quad.placed(source, &at());
        assert!(
            close(drawn.rect, was.rect)
                && close(drawn.uv, was.uv)
                && drawn.clip == was.clip
                && (drawn.rotation - was.rotation).abs() < 1e-5
                && (drawn.opacity, drawn.corner_radius) == (was.opacity, was.corner_radius),
            "{drawn:?} draws as {was:?} did"
        );
    }
}

fn placement(surface: u32) -> ScreenPlacement {
    ScreenPlacement {
        screen: ScreenId(u64::from(surface)),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        surface,
        width: 200,
        height: 100,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    }
}

fn blit(shared: &Rc<RefCell<Shared>>, surface: u32, rect: Rect) -> Blit {
    Blit {
        surface: 7,
        status: PresenterStatus::waiting(),
        shared: Rc::clone(shared),
        screen: ScreenId(u64::from(surface)),
        quad: Quad::upright(rect),
        source: WHOLE,
        zoom: 1.0,
        picture: false,
        placed: None,
        layer: None,
    }
}

#[test]
fn the_webs_frame_stacks_screens_under_the_host_and_anchored_views_and_what_they_hold_above() {
    let shared = Rc::new(RefCell::new(Shared::default()));
    shared.borrow_mut().paint(&[placement(1), placement(2), placement(3)]);
    shared.borrow_mut().publish(
        &ScreenLayout {
            screens: vec![placement(1), placement(2), placement(3)],
        },
        Vec::new(),
        None,
    );
    let editor = blit(&shared, 1, Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0)));
    let popover = Blit {
        layer: Some(AnchoredSource {
            surface: 9,
            pixels: Rect::from_min_size(Pos2::ZERO, vec2(60.0, 60.0)),
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(400.0, 70.0)),
        }),
        ..blit(&shared, 9, Rect::from_min_size(pos2(20.0, 20.0), vec2(60.0, 60.0)))
    };
    let child = blit(&shared, 2, Rect::from_min_size(pos2(30.0, 30.0), vec2(20.0, 10.0)));
    let beside = blit(&shared, 3, Rect::from_min_size(pos2(300.0, 0.0), vec2(200.0, 100.0)));
    let frame = frame(&[editor, popover, child, beside], vec2(800.0, 600.0));
    let surfaces: Vec<Surface> = frame.layers.iter().map(|layer| layer.surface).collect();
    assert_eq!(
        surfaces,
        vec![
            Surface::Screen {
                instance: 7,
                surface: 1
            },
            Surface::Screen {
                instance: 7,
                surface: 3
            },
            Surface::Root(0),
            Surface::Anchored {
                instance: 7,
                surface: 9
            },
            Surface::Screen {
                instance: 7,
                surface: 2
            },
        ],
        "the child placed in the popover goes above it, and the pane beside it stays under the host"
    );
    let anchored = &frame.layers[3];
    assert_eq!(
        (anchored.bounds(), anchored.clip),
        (
            Rect::from_min_max(pos2(20.0, 20.0), pos2(80.0, 70.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(60.0, 50.0))
        ),
        "the web keeps an anchored view's piece to the view's bounds"
    );
}
