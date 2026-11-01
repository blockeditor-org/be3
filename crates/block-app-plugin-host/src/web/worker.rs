use stable_hash::HashSet;
use std::sync::{Arc, atomic::AtomicBool};

use wasm_bindgen::{JsCast, prelude::*};

use super::canvases::{Canvases, Placement};
use super::overlay::{self, Overlay};
use block_guest_host::{
    Channel, Presented, Received,
    web::{Pictures, WebGuest, get, message, set},
};

pub struct PresentedBitmap {
    pub surface: u32,
    pub bitmap: web_sys::ImageBitmap,
}

impl Presented for PresentedBitmap {
    fn surface(&self) -> u32 {
        self.surface
    }
}

impl Pictures for PresentedBitmap {
    fn read(report: &JsValue) -> Vec<Self> {
        js_sys::Array::from(&get(report, "pictures"))
            .iter()
            .filter_map(|picture| {
                Some(PresentedBitmap {
                    surface: get(&picture, "surface").as_f64()? as u32,
                    bitmap: get(&picture, "bitmap").dyn_into().ok()?,
                })
            })
            .collect()
    }
}

pub struct WorkerChannel {
    guest: WebGuest<PresentedBitmap>,
    canvases: Canvases,
    overlay: Option<overlay::Shown>,
    exported: HashSet<u32>,
}

impl WorkerChannel {
    pub(super) fn start(url: &str, waiting: Arc<AtomicBool>) -> Result<Self, String> {
        let guest = WebGuest::start(
            url,
            Some("./plugin.js"),
            Arc::new(block_app_base::host::wake),
            waiting,
        )?;
        Ok(Self {
            guest,
            canvases: Canvases::default(),
            overlay: None,
            exported: HashSet::default(),
        })
    }

    pub fn offer(&self, image: u64, bitmap: &web_sys::ImageBitmap) {
        let message = message("import");
        set(&message, "image", &(image as f64).into());
        set(&message, "bitmap", bitmap);
        self.guest.post(&message, None);
    }

    pub fn withdraw(&self, image: u64) {
        let message = message("unimport");
        set(&message, "image", &(image as f64).into());
        self.guest.post(&message, None);
    }

    pub fn export(&mut self, surfaces: HashSet<u32>) {
        if self.exported == surfaces {
            return;
        }
        let listed = js_sys::Array::new();
        for surface in &surfaces {
            listed.push(&(*surface).into());
        }
        self.exported = surfaces;
        let message = message("export");
        set(&message, "surfaces", &listed);
        self.guest.post(&message, None);
    }

    pub fn place(
        &mut self,
        placements: Vec<Placement>,
        layers: Option<Overlay>,
    ) -> Result<(), String> {
        let mut canvases = std::mem::take(&mut self.canvases);
        let placed = canvases.place(self, placements);
        self.canvases = canvases;
        let mut shown = self.overlay.take();
        let layered = overlay::show(self, &mut shown, layers);
        self.overlay = shown;
        placed.and(layered)
    }

    fn post_canvas(
        &self,
        message: &js_sys::Object,
        canvas: Option<&web_sys::OffscreenCanvas>,
    ) -> bool {
        if let Some(canvas) = canvas {
            set(message, "canvas", canvas);
        }
        self.guest
            .post(message, canvas.map(|canvas| canvas.as_ref()))
    }

    pub(super) fn layers(
        &self,
        canvas: Option<&web_sys::OffscreenCanvas>,
        (width, height): (u32, u32),
        pieces: &[f32],
    ) -> Result<(), String> {
        let message = message("layers");
        set(&message, "width", &width.into());
        set(&message, "height", &height.into());
        set(&message, "pieces", &js_sys::Float32Array::from(pieces));
        self.post_canvas(&message, canvas)
            .then_some(())
            .ok_or_else(|| "a plugin's layers could not be handed to its worker".to_owned())
    }

    pub(super) fn show(
        &self,
        id: u32,
        canvas: Option<&web_sys::OffscreenCanvas>,
        surface: u32,
    ) -> Result<(), String> {
        let message = message("show");
        set(&message, "id", &id.into());
        set(&message, "surface", &surface.into());
        self.post_canvas(&message, canvas)
            .then_some(())
            .ok_or_else(|| "a plugin screen could not be handed to its worker".to_owned())
    }

    pub(super) fn forget(&self, id: u32) {
        let message = message("forget");
        set(&message, "id", &id.into());
        self.guest.post(&message, None);
    }
}

impl Drop for WorkerChannel {
    fn drop(&mut self) {
        self.canvases.clear();
        if let Some(shown) = self.overlay.take() {
            shown.remove();
        }
    }
}

impl Channel for WorkerChannel {
    type Presented = PresentedBitmap;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        self.guest.step(frames)
    }

    fn take_woken(&mut self) -> bool {
        self.guest.take_woken()
    }

    fn receive(&mut self) -> Received<PresentedBitmap> {
        self.guest.receive()
    }
}
