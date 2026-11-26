use be_lib_stable_hash::HashMap;
use std::cell::RefCell;

use be_lib_layers::{LayerId, Surface, WindowLayers};
use beui::Color32;
use wasm_bindgen::JsCast;


const LAYER: &str = "apps";
/// Where a screen goes that lies above the host's canvas in the frame's layers: a screen over an
/// anchored view painted before it, in the anchored views' layer (overlay.rs), so a child placed
/// in an anchored view shows over it (`presenter::lifted`).
const ABOVE: &str = "layers";

/// Where a surface shows: a canvas for the layer `layer`, in `#apps` below the host's canvas or
/// in `#layers` above it (`above`), styled by `style` (`canvas_style`).
pub struct CanvasPlacement<S> {
    pub layer: LayerId,
    pub surface: S,
    pub style: String,
    pub above: bool,
}

/// Whoever draws into the canvases a set of `Canvases` shows: a runtime's worker, or under a
/// root (be-host's `?root=`) the root's page and its apps' workers.
pub trait DrawsCanvases {
    /// What a canvas shows, as its placements name it.
    type Surface: Copy + PartialEq;

    /// Hands `canvas`, the canvas `id`, to whoever draws `surface` into it.
    fn show(
        &self,
        id: u32,
        canvas: &web_sys::OffscreenCanvas,
        surface: Self::Surface,
    ) -> Result<(), String>;

    /// Has the canvas `id`, which showed `from`, show `to` instead, keeping what it shows until
    /// `to` is drawn into it: None where it cannot (`to` is drawn by someone else), so `to` shows
    /// in another canvas.
    fn retarget(&self, id: u32, from: Self::Surface, to: Self::Surface) -> Option<Result<(), String>>;

    /// Stops drawing into the canvas `id`, which showed `surface`.
    fn forget(&self, id: u32, surface: Self::Surface);

    /// Which kind of canvas shows `surface`, as its `data-kind` names it for whatever inspects
    /// the page (crates/be-host/web-test).
    fn kind(&self, surface: Self::Surface) -> &'static str;
}

/// Where each runtime's screens show, by its slot: a canvas for each of the frame's screen
/// layers, in `#apps` below the host's canvas (`Surface::Root`) or in `#layers` above it, stacked
/// by their place in the list.
pub fn placements(layers: &WindowLayers, scale: f32) -> HashMap<u64, Vec<CanvasPlacement<u32>>> {
    let mut placed = HashMap::<u64, Vec<CanvasPlacement<u32>>>::default();
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
        placed.entry(instance).or_default().push(CanvasPlacement {
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

struct Shown<S> {
    id: u32,
    element: web_sys::HtmlCanvasElement,
    surface: S,
    style: String,
    above: bool,
}

/// The canvases a runtime's screens show in, or under a root the root's frame (be-host's
/// `?root=`). A canvas keeps what was last drawn into it, so a canvas whose layer leaves the list
/// moves to a new layer showing the same surface (a view mounted anew), and one whose surface is
/// still retained (a runtime's `Run::retained_surfaces`: the app's layout, and the kept frames of
/// views it left out, `Shown::keep_left_out`, such as a dock tab or bar hidden with its tab; under
/// a root, a surface its app has not let go of) stays on the page, hidden, until a layer shows
/// that surface again or it is let go, as natively the presenter keeps its texture: shown again,
/// a view shows its last frame at once rather than an empty canvas until its drawer paints it
/// (guides/apps/runtime.md, Layout and present commits).
pub struct Canvases<S> {
    shown: HashMap<LayerId, Shown<S>>,
    hidden: Vec<Shown<S>>,
}

impl<S> Default for Canvases<S> {
    fn default() -> Self {
        Self {
            shown: HashMap::default(),
            hidden: Vec::new(),
        }
    }
}

/// The style of a canvas kept while its view is hidden.
const HIDDEN: &str = "display: none";

impl<S: Copy + PartialEq> Canvases<S> {
    /// DrawsCanvases each of `placements` in a canvas, keeping hidden the canvases of the surfaces
    /// `retained` holds that no placement shows. A layer that shows another surface than it did
    /// keeps its canvas, which shows what it did until the new surface is drawn into it (a view
    /// shown again on a surface of its own), where its drawer can (`retarget`). Answers the
    /// surfaces that could not be shown, and why.
    pub fn place<D: DrawsCanvases<Surface = S>>(
        &mut self,
        drawer: &D,
        placements: Vec<CanvasPlacement<S>>,
        retained: impl Fn(S) -> bool,
    ) -> Vec<(S, String)> {
        let mut placed = HashMap::with_capacity_and_hasher(placements.len(), Default::default());
        let mut failed = Vec::new();
        let mut unplaced = Vec::new();
        for placement in placements {
            match self.shown.remove(&placement.layer) {
                Some(shown) if shown.surface == placement.surface => {
                    let key = placement.layer;
                    placed.insert(key, update(shown, placement, &mut failed));
                }
                Some(mut shown) => match drawer.retarget(shown.id, shown.surface, placement.surface) {
                    Some(retargeted) => {
                        if let Err(error) = retargeted {
                            failed.push((placement.surface, error));
                        }
                        shown.surface = placement.surface;
                        let key = placement.layer;
                        placed.insert(key, update(shown, placement, &mut failed));
                    }
                    None => {
                        self.hidden.push(shown);
                        unplaced.push(placement);
                    }
                },
                None => unplaced.push(placement),
            }
        }
        let mut left: Vec<Shown<S>> = self.shown.drain().map(|(_, shown)| shown).collect();
        left.append(&mut self.hidden);
        for placement in unplaced {
            let reused = left
                .iter()
                .position(|shown| shown.surface == placement.surface)
                .map(|at| left.swap_remove(at));
            let shown = match reused {
                Some(shown) => shown,
                None => match create(drawer, placement.surface, placement.above) {
                    Ok(shown) => shown,
                    Err(error) => {
                        failed.push((placement.surface, error));
                        continue;
                    }
                },
            };
            let key = placement.layer;
            placed.insert(key, update(shown, placement, &mut failed));
        }
        for mut shown in left {
            let hides = retained(shown.surface)
                && !self
                    .hidden
                    .iter()
                    .any(|hidden| hidden.surface == shown.surface);
            if !hides {
                shown.element.remove();
                drawer.forget(shown.id, shown.surface);
                continue;
            }
            if shown.style != HIDDEN {
                let _ = shown.element.set_attribute("style", HIDDEN);
                shown.style = HIDDEN.to_owned();
            }
            self.hidden.push(shown);
        }
        self.shown = placed;
        failed
    }

    /// Takes every canvas off the page, and `drawer`, if any is left, forgets each.
    pub fn clear<D: DrawsCanvases<Surface = S>>(&mut self, drawer: Option<&D>) {
        let shown = self.shown.drain().map(|(_, shown)| shown);
        for shown in shown.chain(self.hidden.drain(..)) {
            shown.element.remove();
            if let Some(drawer) = drawer {
                drawer.forget(shown.id, shown.surface);
            }
        }
    }
}

/// Moves `shown` to `placement`: its layer above or below the host's canvas, and its style.
fn update<S: Copy>(
    mut shown: Shown<S>,
    placement: CanvasPlacement<S>,
    failed: &mut Vec<(S, String)>,
) -> Shown<S> {
    if shown.above != placement.above {
        // Moving a canvas keeps the offscreen canvas its drawer draws into.
        match layer(placement.above) {
            Ok(layer) => {
                let _ = layer.append_child(&shown.element);
                shown.above = placement.above;
            }
            Err(error) => failed.push((placement.surface, error)),
        }
    }
    if shown.style != placement.style {
        let _ = shown.element.set_attribute("style", &placement.style);
        shown.style = placement.style;
    }
    shown
}

fn create<D: DrawsCanvases>(drawer: &D, surface: D::Surface, above: bool) -> Result<Shown<D::Surface>, String> {
    thread_local! {
        static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let failed = |_| "a canvas could not be made".to_owned();
    let layer = layer(above)?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("the page has no document")?;
    let element: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(failed)?
        .dyn_into()
        .map_err(|_| "a canvas could not be made".to_owned())?;
    let _ = element.set_attribute("data-kind", drawer.kind(surface));
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
        .map_err(|_| "a canvas could not be handed to whoever draws it".to_owned());
    match offscreen.and_then(|offscreen| drawer.show(id, &offscreen, surface)) {
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
