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
    let mut placed = HashMap::<u64, Vec<Placement>>::default();
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

/// The canvases a runtime's screens show in. A canvas keeps what its worker last drew into it,
/// so a canvas whose layer leaves the list moves to a new layer showing the same surface (a view
/// mounted anew), and one whose surface the runtime still retains (`Run::retained_surfaces`: the
/// app's layout, and the kept frames of views it left out, `Shown::keep_left_out`, such as a dock
/// tab or bar hidden with its tab) stays on the page, hidden, until a layer shows that surface
/// again or the runtime lets it go, as natively the presenter keeps its texture: shown again
/// before its app has drawn it anew, a view shows its last frame rather than an empty canvas
/// (guides/apps/runtime.md, Layout and present commits).
#[derive(Default)]
pub(super) struct Canvases {
    shown: HashMap<LayerId, Shown>,
    hidden: Vec<Shown>,
}

/// The style of a canvas kept while its view is hidden.
const HIDDEN: &str = "display: none";

impl Canvases {
    /// Shows each of `placements` in a canvas, keeping hidden the canvases of the surfaces in
    /// `retained` that no placement shows.
    pub(super) fn place(
        &mut self,
        channel: &WorkerChannel,
        placements: Vec<Placement>,
        retained: &[u32],
    ) -> Result<(), String> {
        let mut placed = HashMap::with_capacity_and_hasher(placements.len(), Default::default());
        let mut result = Ok(());
        let mut unplaced = Vec::new();
        for placement in placements {
            match self.shown.remove(&placement.layer) {
                Some(shown) => {
                    let key = placement.layer;
                    placed.insert(key, update(channel, shown, placement, &mut result));
                }
                None => unplaced.push(placement),
            }
        }
        let mut left: Vec<Shown> = self.shown.drain().map(|(_, shown)| shown).collect();
        left.append(&mut self.hidden);
        for placement in unplaced {
            let reused = left
                .iter()
                .position(|shown| shown.surface == placement.surface)
                .map(|at| left.swap_remove(at));
            let shown = match reused {
                Some(shown) => shown,
                None => match create(channel, placement.surface, placement.above) {
                    Ok(shown) => shown,
                    Err(error) => {
                        result = result.and(Err(error));
                        continue;
                    }
                },
            };
            let key = placement.layer;
            placed.insert(key, update(channel, shown, placement, &mut result));
        }
        for mut shown in left {
            let hides = retained.contains(&shown.surface)
                && !self.hidden.iter().any(|hidden| hidden.surface == shown.surface);
            if !hides {
                shown.element.remove();
                channel.forget(shown.id);
                continue;
            }
            if shown.style != HIDDEN {
                let _ = shown.element.set_attribute("style", HIDDEN);
                shown.style = HIDDEN.to_owned();
            }
            self.hidden.push(shown);
        }
        self.shown = placed;
        result
    }

    pub(super) fn clear(&mut self) {
        for shown in self.shown.drain().map(|(_, shown)| shown) {
            shown.element.remove();
        }
        for shown in self.hidden.drain(..) {
            shown.element.remove();
        }
    }
}

/// Moves `shown` to `placement`: its surface, its layer above or below the host's canvas, and its
/// style.
fn update(
    channel: &WorkerChannel,
    mut shown: Shown,
    placement: Placement,
    result: &mut Result<(), String>,
) -> Shown {
    if shown.surface != placement.surface {
        shown.surface = placement.surface;
        let told = channel.show(shown.id, None, placement.surface);
        if result.is_ok() {
            *result = told;
        }
    }
    if shown.above != placement.above {
        // Moving a canvas keeps the offscreen canvas its worker draws into.
        match layer(placement.above) {
            Ok(layer) => {
                let _ = layer.append_child(&shown.element);
                shown.above = placement.above;
            }
            Err(error) if result.is_ok() => *result = Err(error),
            Err(_) => {}
        }
    }
    if shown.style != placement.style {
        let _ = shown.element.set_attribute("style", &placement.style);
        shown.style = placement.style;
    }
    shown
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
