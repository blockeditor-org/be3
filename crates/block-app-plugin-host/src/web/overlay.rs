use stable_hash::HashMap;

use beui::{Rect, Vec2};
use wasm_bindgen::JsCast;

use super::worker::WorkerChannel;
use crate::presenter::Blit;

const LAYER: &str = "layers";

#[derive(Clone, PartialEq)]
pub struct Overlay {
    size: (u32, u32),
    pieces: Vec<f32>,
    style: String,
}

pub fn overlays(
    blits: &[Blit],
    viewport: Vec2,
    pixels_per_point: f32,
    scale: f32,
) -> HashMap<u32, Overlay> {
    let mut owners = Vec::<u32>::new();
    let mut pieces = HashMap::<u32, Vec<f32>>::default();
    let mut bounds = HashMap::<u32, Rect>::default();
    for blit in blits {
        let Some(layer) = blit.layer else {
            continue;
        };
        owners.retain(|owner| *owner != blit.surface);
        owners.push(blit.surface);
        let target = blit.quad.rect;
        pieces.entry(blit.surface).or_default().extend([
            layer.surface as f32,
            target.min.x * pixels_per_point,
            target.min.y * pixels_per_point,
            target.max.x * pixels_per_point,
            target.max.y * pixels_per_point,
            layer.pixels.min.x,
            layer.pixels.min.y,
            layer.pixels.max.x,
            layer.pixels.max.y,
        ]);
        bounds
            .entry(blit.surface)
            .and_modify(|bounds| *bounds = bounds.union(layer.bounds))
            .or_insert(layer.bounds);
    }
    let size = (
        (viewport.x * pixels_per_point).ceil().max(1.0) as u32,
        (viewport.y * pixels_per_point).ceil().max(1.0) as u32,
    );
    let (width, height) = (viewport.x * scale, viewport.y * scale);
    owners
        .into_iter()
        .enumerate()
        .map(|(order, owner)| {
            let bounds = bounds[&owner];
            let style = format!(
                "width:{width}px;height:{height}px;z-index:{};clip-path:inset({}px {}px {}px {}px)",
                order + 1,
                (bounds.min.y * scale).max(0.0),
                (width - bounds.max.x * scale).max(0.0),
                (height - bounds.max.y * scale).max(0.0),
                (bounds.min.x * scale).max(0.0),
            );
            let overlay = Overlay {
                size,
                pieces: pieces.remove(&owner).unwrap_or_default(),
                style,
            };
            (owner, overlay)
        })
        .collect()
}

pub(super) struct Shown {
    element: web_sys::HtmlCanvasElement,
    overlay: Overlay,
}

impl Shown {
    pub(super) fn remove(self) {
        self.element.remove();
    }
}

pub(super) fn show(
    channel: &WorkerChannel,
    shown: &mut Option<Shown>,
    overlay: Option<Overlay>,
) -> Result<(), String> {
    let overlay = match (overlay, shown.as_ref()) {
        (Some(overlay), _) => overlay,
        (None, None) => return Ok(()),
        (None, Some(held)) => Overlay {
            size: held.overlay.size,
            pieces: Vec::new(),
            style: "visibility:hidden".to_owned(),
        },
    };
    if let Some(held) = shown {
        if held.overlay == overlay {
            return Ok(());
        }
        if held.overlay.style != overlay.style {
            let _ = held.element.set_attribute("style", &overlay.style);
        }
        if (held.overlay.size, &held.overlay.pieces) != (overlay.size, &overlay.pieces) {
            channel.layers(None, overlay.size, &overlay.pieces)?;
        }
        held.overlay = overlay;
        return Ok(());
    }
    let failed = |_| "a plugin's layer canvas could not be made".to_owned();
    let layer = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(LAYER))
        .ok_or_else(|| format!("the page has no #{LAYER} element for plugin layers to show in"))?;
    let element: web_sys::HtmlCanvasElement = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("the page has no document")?
        .create_element("canvas")
        .map_err(failed)?
        .dyn_into()
        .map_err(|_| "a plugin's layer canvas could not be made".to_owned())?;
    let _ = element.set_attribute("style", &overlay.style);
    layer.append_child(&element).map_err(failed)?;
    let offscreen = element
        .transfer_control_to_offscreen()
        .map_err(|_| "a plugin's layer canvas could not be handed to its worker".to_owned());
    match offscreen
        .and_then(|offscreen| channel.layers(Some(&offscreen), overlay.size, &overlay.pieces))
    {
        Ok(()) => {
            *shown = Some(Shown { element, overlay });
            Ok(())
        }
        Err(error) => {
            element.remove();
            Err(error)
        }
    }
}
