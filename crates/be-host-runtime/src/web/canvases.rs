use be_lib_stable_hash::HashMap;
use std::cell::RefCell;

use be_sdk_api::ScreenId;
use beui::Color32;
use wasm_bindgen::JsCast;

use super::worker::WorkerChannel;
use crate::presenter::Blit;

const LAYER: &str = "apps";
/// Where a screen goes that lies over an anchored view painted before it: above the host's
/// canvas, in the anchored views' layer (overlay.rs), so a child placed in an anchored view
/// shows over it (`presenter::lifted`).
const ABOVE: &str = "layers";

pub struct Placement {
    screen: ScreenId,
    surface: u32,
    style: String,
    above: bool,
}

/// Where a screen shows: `order` is its blit's place in the frame's paint order, and `above`
/// whether it lies over an anchored view painted before it (`presenter::lifted`).
pub fn placement(blit: &Blit, order: usize, above: bool, scale: f32) -> Option<Placement> {
    if blit.layer.is_some() {
        return None;
    }
    let shared = blit.shared.borrow();
    let placement = shared.placement(blit.screen)?;
    let painted = shared.holds(placement, None)?;
    let (quad, source) = blit.shown_at(painted)?;
    let style = crate::presenter::canvas_style(quad, source, painted.size, scale, order)?;
    Some(Placement {
        screen: blit.screen,
        surface: placement.surface,
        style,
        above,
    })
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
    shown: HashMap<(ScreenId, usize), Shown>,
}

impl Canvases {
    pub(super) fn place(
        &mut self,
        channel: &WorkerChannel,
        placements: Vec<Placement>,
    ) -> Result<(), String> {
        let mut seen = HashMap::<ScreenId, usize>::default();
        let mut kept = HashMap::with_capacity_and_hasher(placements.len(), Default::default());
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
