use std::{cell::RefCell, collections::HashMap};

use beui::{Color32, pos2};
use block_plugin_api::ScreenId;
use wasm_bindgen::JsCast;

use super::worker::WorkerChannel;
use crate::presenter::Blit;

const LAYER: &str = "plugins";

pub struct Placement {
    screen: ScreenId,
    surface: u32,
    style: String,
}

pub fn placement(blit: &Blit, order: usize, scale: f32) -> Option<Placement> {
    if blit.layer.is_some() {
        return None;
    }
    let shared = blit.shared.borrow();
    let placement = shared.layout.placement(blit.screen)?;
    if placement.width == 0 || placement.height == 0 {
        return None;
    }
    let (quad, source) = blit.shown_at((placement.width, placement.height))?;
    if !source.is_positive() {
        return None;
    }
    let [first, second, _, fourth] = quad
        .corners
        .map(|corner| pos2(corner.x * scale, corner.y * scale));
    let across = (second - first) / source.width();
    let down = (fourth - first) / source.height();
    let origin = first - across * source.min.x - down * source.min.y;
    let (width, height) = (across.length(), down.length());
    if width <= f32::EPSILON || height <= f32::EPSILON {
        return None;
    }
    let percent = |value: f32| (value * 100.0).clamp(0.0, 100.0);
    let radius = quad.corner_radius.max(0.0) * scale;
    let round = |left: bool, top: bool| {
        let reaches = |near: bool, min: f32, max: f32| match near {
            true => min <= 0.0,
            false => max >= 1.0,
        };
        match reaches(left, source.min.x, source.max.x) && reaches(top, source.min.y, source.max.y)
        {
            true => radius,
            false => 0.0,
        }
    };
    let style = format!(
        "width:{width}px;height:{height}px;\
         transform:matrix({},{},{},{},{},{});\
         clip-path:inset({}% {}% {}% {}% round {}px {}px {}px {}px);opacity:{};z-index:{order}",
        across.x / width,
        across.y / width,
        down.x / height,
        down.y / height,
        origin.x,
        origin.y,
        percent(source.min.y),
        percent(1.0 - source.max.x),
        percent(1.0 - source.max.y),
        percent(source.min.x),
        round(true, true),
        round(false, true),
        round(false, false),
        round(true, false),
        quad.opacity.clamp(0.0, 1.0),
    );
    Some(Placement {
        screen: blit.screen,
        surface: placement.surface,
        style,
    })
}

pub fn set_background(color: Color32) {
    thread_local! {
        static SHOWN: RefCell<Option<Color32>> = const { RefCell::new(None) };
    }
    if SHOWN.with(|shown| shown.replace(Some(color))) == Some(color) {
        return;
    }
    let Ok(layer) = layer() else {
        return;
    };
    let [red, green, blue, _] = color.to_array();
    let _ = layer
        .style()
        .set_property("background", &format!("rgb({red},{green},{blue})"));
}

struct Shown {
    id: u32,
    element: web_sys::HtmlCanvasElement,
    surface: u32,
    style: String,
}

#[derive(Default)]
pub(super) struct Canvases {
    shown: HashMap<(ScreenId, usize), Shown>,
}

impl Canvases {
    pub(super) fn place(
        &mut self,
        channel: &WorkerChannel,
        placements: Vec<Placement>,
    ) -> Result<(), String> {
        let mut seen = HashMap::<ScreenId, usize>::new();
        let mut kept = HashMap::with_capacity(placements.len());
        let mut result = Ok(());
        for placement in placements {
            let occurrence = seen.entry(placement.screen).or_default();
            let key = (placement.screen, *occurrence);
            *occurrence += 1;
            let mut shown = match self.shown.remove(&key) {
                Some(mut shown) => {
                    if shown.surface != placement.surface {
                        shown.surface = placement.surface;
                        result = result.and(channel.show(shown.id, None, placement.surface));
                    }
                    shown
                }
                None => match create(channel, placement.surface) {
                    Ok(shown) => shown,
                    Err(error) => {
                        result = result.and(Err(error));
                        continue;
                    }
                },
            };
            if shown.style != placement.style {
                let _ = shown.element.set_attribute("style", &placement.style);
                shown.style = placement.style;
            }
            kept.insert(key, shown);
        }
        for shown in std::mem::replace(&mut self.shown, kept).into_values() {
            shown.element.remove();
            channel.forget(shown.id);
        }
        result
    }

    pub(super) fn clear(&mut self) {
        for shown in self.shown.drain().map(|(_, shown)| shown) {
            shown.element.remove();
        }
    }
}

fn create(channel: &WorkerChannel, surface: u32) -> Result<Shown, String> {
    thread_local! {
        static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let failed = |_| "a plugin screen's canvas could not be made".to_owned();
    let layer = layer()?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("the page has no document")?;
    let element: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(failed)?
        .dyn_into()
        .map_err(|_| "a plugin screen's canvas could not be made".to_owned())?;
    layer.append_child(&element).map_err(failed)?;
    let id = NEXT.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1));
        id
    });
    let shown = Shown {
        id,
        element,
        surface,
        style: String::new(),
    };
    let offscreen = shown
        .element
        .transfer_control_to_offscreen()
        .map_err(|_| "a plugin screen's canvas could not be handed to its worker".to_owned());
    match offscreen.and_then(|offscreen| channel.show(id, Some(&offscreen), surface)) {
        Ok(()) => Ok(shown),
        Err(error) => {
            shown.element.remove();
            Err(error)
        }
    }
}

fn layer() -> Result<web_sys::HtmlElement, String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(LAYER))
        .and_then(|layer| layer.dyn_into().ok())
        .ok_or_else(|| format!("the page has no #{LAYER} element for plugins to show in"))
}
