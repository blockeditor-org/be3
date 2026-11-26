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
        composited: false,
    }
}

fn close(one: [f32; 4], other: [f32; 4]) -> bool {
    one.iter()
        .zip(other)
        .all(|(one, other)| (one - other).abs() < 1e-3)
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

mod a_layer_draws_natively_as_the_quad_its_blit_drew;
mod a_views_layer_keeps_its_id_when_a_blit_before_it_goes;
mod the_webs_frame_stacks_screens_under_the_host_and_anchored_views_and_what_they_hold_above;
