use be_lib_stable_hash::HashMap;
use std::cell::RefCell;

use be_lib_layers::{LayerId, Surface, WindowLayers};
use beui::Color32;
use wasm_bindgen::JsCast;

use super::worker::WorkerChannel;

const LAYER: &str = "apps";
/// Where a screen goes that lies above the host's canvas in the frame's layers: a screen over an
/// anchored view painted before it, in the anchored views' layer (overlay.rs), so a child placed
/// in an anchored view shows over it (`presenter::lifted`).
const ABOVE: &str = "layers";

pub struct Placement {
    layer: LayerId,
    surface: u32,
    style: String,
    above: bool,
}

/// Where each runtime's screens show, by its slot: a canvas for each of the frame's screen
/// layers, in `#apps` below the host's canvas (`Surface::Root`) or in `#layers` above it, stacked
/// by their place in the list.
pub fn placements(layers: &WindowLayers, scale: f32) -> HashMap<u64, Vec<Placement>> {
    let mut placed = HashMap::<u32, Vec<Placement>>::default();
    let mut above = false;
    for (order, layer) in layers.layers.iter().enumerate() {
        let (instance, surface) = match layer.surface {
            Surface::Root(_) => {
                above = true;
                continue;
            }
            Surface::Screen { instance, surface } => (instance, surface),
            Surface::Anchored { .. } | Surface::Host(_) => continue,
        };
        let Some(style) = crate::layers::canvas_style(layer, scale, order) else {
            continue;
        };
        placed.entry(instance).or_default().push(Placement {
            layer: layer.id,
            surface,
            style,
            above,
        });
    }
    placed
}

pub fn set_background(color: Color32) {
    thread_local! {
        static SHOWN: RefCell<Option<Color32>> = const { RefCell::new(None) };
    }
    if SHOWN.with(|shown| shown.replace(Some(color))) == Some(color) {
        return;
    }
    let Ok(layer) = layer(false) else {
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
    above: bool,
}

#[derive(Default)]
pub(super) struct Canvases {
    shown: HashMap<LayerId, Shown>,
}

impl Canvases {
    pub(super) fn place(
        &mut self,
        channel: &WorkerChannel,
        placements: Vec<Placement>,
    ) -> Result<(), String> {
        let mut kept = HashMap::with_capacity_and_hasher(placements.len(), Default::default());
        let mut result = Ok(());
        for placement in placements {
            let key = placement.layer;
            let mut shown = match self.shown.remove(&key) {
                Some(mut shown) => {
                    if shown.surface != placement.surface {
                        shown.surface = placement.surface;
                        result = result.and(channel.show(shown.id, None, placement.surface));
                    }
                    shown
                }
                None => match create(channel, placement.surface, placement.above) {
                    Ok(shown) => shown,
                    Err(error) => {
                        result = result.and(Err(error));
                        continue;
                    }
                },
            };
            if shown.above != placement.above {
                // Moving a canvas keeps the offscreen canvas its worker draws into.
                match layer(placement.above) {
                    Ok(layer) => {
                        let _ = layer.append_child(&shown.element);
                        shown.above = placement.above;
                    }
                    Err(error) => result = result.and(Err(error)),
                }
            }
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

fn create(channel: &WorkerChannel, surface: u32, above: bool) -> Result<Shown, String> {
    thread_local! {
        static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let failed = |_| "an app screen's canvas could not be made".to_owned();
    let layer = layer(above)?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("the page has no document")?;
    let element: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(failed)?
        .dyn_into()
        .map_err(|_| "an app screen's canvas could not be made".to_owned())?;
    // Which kind of canvas this is, for whatever inspects the page (crates/be-host/web-test).
    let _ = element.set_attribute("data-kind", "screen");
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
        above,
    };
    let offscreen = shown
        .element
        .transfer_control_to_offscreen()
        .map_err(|_| "an app screen's canvas could not be handed to its worker".to_owned());
    match offscreen.and_then(|offscreen| channel.show(id, Some(&offscreen), surface)) {
        Ok(()) => Ok(shown),
        Err(error) => {
            shown.element.remove();
            Err(error)
        }
    }
}

fn layer(above: bool) -> Result<web_sys::HtmlElement, String> {
    let id = match above {
        true => ABOVE,
        false => LAYER,
    };
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|layer| layer.dyn_into().ok())
        .ok_or_else(|| format!("the page has no #{id} element for apps to show in"))
}
