//! What a root's frame shows on the web (`?root=APP`): each layer of its frame in a canvas of
//! the page's, in `#layers` above the host's canvas, placed by its layer's transform, clip and
//! opacity and stacked by its place in the list (be-host-runtime's `canvas_style`, as an app's
//! screens are placed under be-host's own UI). The root's own surfaces draw into theirs on the
//! page's gpu shim (`Root::show_surface`, `paint`), and each screen of an app it spawned is drawn
//! by that app's worker, which its canvas is handed to (`Root::show_app_surface`). They are
//! be-host-runtime's `Canvases`, as an app's screens are under be-host's own UI: a canvas whose
//! layer leaves the list stays hidden while its app holds its surface
//! (`Root::holds_app_surface`). The root loads asynchronously (`load`), and its frames answer
//! once its children did (`frame`).

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    time::Instant,
};

use be_host_runtime::{CanvasPlacement, Canvases, DrawsCanvases};
use be_lib_layers::{LayerList, Surface};
use be_wasm_root_api::{FrameDone, FrameRequest};
use be_wasm_root_host::Root;
use beui::reactive::*;
use beui::styled::*;

use super::SHOWN;
use super::serving::fonts;

/// What a canvas shows: a surface of the root's own, or a screen or anchored view of an app it
/// spawned.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Root(u32),
    App {
        instance: u64,
        surface: u32,
        anchored: bool,
    },
}

/// The root, drawing into the canvases of its frame: its own surfaces on the page's gpu shim, and
/// its apps' screens and anchored views in their workers.
struct Drawer<'a>(&'a Root);

impl DrawsCanvases for Drawer<'_> {
    type Surface = Key;

    fn show(&self, id: u32, canvas: &web_sys::OffscreenCanvas, key: Key) -> Result<(), String> {
        match key {
            Key::Root(surface) => {
                self.0.show_surface(id, canvas, surface);
                Ok(())
            }
            Key::App {
                instance, surface, ..
            } => self.0.show_app_surface(id, canvas, instance, surface),
        }
    }

    /// A canvas of an app's stays its worker's: it shows another surface of the same app's.
    fn retarget(&self, id: u32, from: Key, to: Key) -> Option<Result<(), String>> {
        match (from, to) {
            (
                Key::App { instance, .. },
                Key::App {
                    instance: to,
                    surface,
                    ..
                },
            ) if instance == to => Some(self.0.retarget_app_surface(id, instance, surface)),
            _ => None,
        }
    }

    fn forget(&self, id: u32, key: Key) {
        match key {
            Key::Root(_) => self.0.forget_surface(id),
            Key::App { instance, .. } => self.0.forget_app_surface(id, instance),
        }
    }

    /// An app's anchored view is an overlay, as be-host's own page names the canvas it draws them
    /// in.
    fn kind(&self, key: Key) -> &'static str {
        match key {
            Key::Root(_) => "root",
            Key::App {
                anchored: false, ..
            } => "screen",
            Key::App { anchored: true, .. } => "overlay",
        }
    }
}

/// The page's canvases for the root's frames, and the root as it loads and answers.
#[derive(Default)]
pub(super) struct Surfaces {
    /// The root once it loaded, or why it did not, until the host takes it.
    loaded: Rc<RefCell<Option<Result<Root, String>>>>,
    /// Whether the root is loading.
    loading: Cell<bool>,
    /// What the root answered a frame with after the host frame that asked for it.
    answered: Rc<RefCell<Option<Result<FrameDone, String>>>>,
    /// Above the host's canvas, which shows only the background, and passing every input through
    /// to it, which the root routes.
    canvases: Canvases<Key>,
    /// The screens of apps that could not be handed a canvas (an app that stopped), until their
    /// layers leave the list: none is made for them again each frame.
    refused: HashSet<Key>,
}

impl Surfaces {
    /// The document the host's canvas shows under the root's canvases: its background alone.
    pub(super) fn document(_desktop: bool) -> (beui::native::Document, Self) {
        let document = beui_host::native::build(|| {
            view! {
                <beui::styled::Root title="be-host">
                    <Drawing @test_id={SHOWN} draw={move || beui::native::draw_gpu(None)} />
                </beui::styled::Root>
            }
        });
        (document, Self::default())
    }

    /// Starts loading `app`, a staged root, from where the page stages its entry point, as the
    /// apps it spawns are found (be-host-runtime's `staged_app`), waking the host once it has.
    pub(super) fn load(&mut self, app: &str, waker: beui::native::Waker) -> Result<(), String> {
        let manifest = super::root_manifest(app)?;
        let url = manifest.entry_point.clone();
        let loaded = Rc::clone(&self.loaded);
        self.loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let root = Root::load(&url, Box::new(be_host_runtime::staged_app))
                .await
                // A page delivers Ctrl+V as a paste, from the browser's paste event.
                .map(|root| root.with_fonts(fonts()).with_paste_shortcut(true));
            if let Ok(root) = &root {
                let waker = waker.clone();
                root.on_wake(move || waker.wake());
            }
            *loaded.borrow_mut() = Some(root);
            waker.wake();
        });
        Ok(())
    }

    /// The root once it loaded, or why it did not, the first time it is asked after.
    pub(super) fn loaded(&self) -> Option<Result<Root, String>> {
        let loaded = self.loaded.borrow_mut().take();
        if loaded.is_some() {
            self.loading.set(false);
        }
        loaded
    }

    /// What the root owes the host now beside what its frames say: its load, the frame it is
    /// drawing, or one it answered that the host has yet to show.
    pub(super) fn busy(&self, root: Option<&Root>) -> Option<&'static str> {
        if self.loading.get() {
            return Some("is loading");
        }
        if self.answered.borrow().is_some() {
            return Some("answered a frame the host has not shown yet");
        }
        root.is_some_and(Root::drawing)
            .then_some("draws a frame its children have not answered yet")
    }

    /// What the root answered for a frame: one it answered after the host frame that asked for
    /// it, else the frame asked for `request` now, if it answers at once. None while it draws,
    /// which wakes the host once it answers.
    pub(super) fn frame(
        &mut self,
        root: &Root,
        request: FrameRequest,
        deadline: Instant,
        waker: Option<beui::native::Waker>,
    ) -> Option<Result<FrameDone, String>> {
        if let Some(done) = self.answered.borrow_mut().take() {
            return Some(done);
        }
        let answered = Rc::clone(&self.answered);
        root.frame(request, deadline, move |done| {
            *answered.borrow_mut() = Some(done);
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        self.answered.borrow_mut().take()
    }

    /// Drive's paintings are not read on the web, which shows what the page shows instead.
    pub(super) fn painted(&mut self, _app: &str, _painted: Vec<u8>) {}

    /// Shows each layer of `list` in the host's one window, which the list names as none, in a
    /// canvas of its own, placed and stacked by the list; a canvas whose layer left the list
    /// stays hidden while its app holds its surface, and otherwise leaves the page. Then draws
    /// the root's own surfaces into theirs.
    pub(super) fn show(
        &mut self,
        root: &Root,
        list: LayerList,
        context: &beui::native::Context,
    ) -> Result<(), String> {
        // CSS pixels a point, as be-host places its apps' screens.
        let scale = context.pixels_per_point() / context.native_pixels_per_point();
        let mut refused = HashSet::new();
        let layers: Vec<_> = list
            .windows
            .into_iter()
            .filter(|window| window.window.is_none())
            .flat_map(|window| window.layers)
            .collect();
        // The surfaces of its apps the list names though it shows them not: the last frames the
        // root keeps of views hidden while their apps draw them anew (be-sdk-root's
        // `Embedded::kept_layers`), whose canvases stay hidden, as natively their textures stay.
        let kept: HashSet<(u64, u32)> = layers
            .iter()
            .filter(|layer| !layer.shown)
            .filter_map(|layer| match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    Some((instance, surface))
                }
                Surface::Root(_) | Surface::Host(_) => None,
            })
            .collect();
        let layers = layers.into_iter().filter(|layer| layer.shown);
        let mut placements = Vec::new();
        for (order, layer) in layers.enumerate() {
            let key = match layer.surface {
                Surface::Root(surface) => Key::Root(surface),
                Surface::Screen { instance, surface } => Key::App {
                    instance,
                    surface,
                    anchored: false,
                },
                Surface::Anchored { instance, surface } => Key::App {
                    instance,
                    surface,
                    anchored: true,
                },
                // Program windows run only natively.
                Surface::Host(_) => continue,
            };
            let Some(style) = be_host_runtime::canvas_style(&layer, scale, order) else {
                continue;
            };
            if self.refused.contains(&key) {
                refused.insert(key);
                continue;
            }
            placements.push(CanvasPlacement {
                layer: layer.id,
                surface: key,
                style,
                above: true,
            });
        }
        let retained = |key| match key {
            Key::App {
                instance, surface, ..
            } => kept.contains(&(instance, surface)) || root.holds_app_surface(instance, surface),
            // The root's own surfaces draw on the page's gpu shim, which paints a canvas made anew
            // with what its surface last showed before the page shows again (`paint`, below),
            // where an app's worker paints its canvas only once it is handed it.
            Key::Root(_) => false,
        };
        for (key, why) in self.canvases.place(&Drawer(root), placements, retained) {
            match key {
                // An app that stopped, which the list names until the root hears so, shows
                // nothing; the root's own surfaces must show.
                Key::App { .. } => {
                    refused.insert(key);
                }
                Key::Root(_) => return Err(why),
            }
        }
        self.refused = refused;
        root.paint().map(|_| ())
    }

    /// Takes every canvas off the page, as the root stops.
    pub(super) fn clear(&mut self, root: Option<&Root>) {
        self.canvases.clear(root.map(Drawer).as_ref());
    }
}

