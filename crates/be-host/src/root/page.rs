//! What a root's frame shows on the web (`?root=APP`): each layer of its frame in a canvas of
//! the page's, in `#layers` above the host's canvas, placed by its layer's transform, clip and
//! opacity and stacked by its place in the list (be-host-runtime's `canvas_style`, as an app's
//! screens are placed under be-host's own UI). The root's own surfaces draw into theirs on the
//! page's gpu shim (`Root::show_surface`, `paint`), and each screen of an app it spawned is drawn
//! by that app's worker, which its canvas is handed to (`Root::show_app_surface`). The root loads
//! asynchronously (`load`), and its frames answer once its children did (`frame`).

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Instant,
};

use be_lib_layers::{LayerList, Surface};
use be_wasm_root_api::{FrameDone, FrameRequest};
use be_wasm_root_host::Root;
use beui::reactive::*;
use beui::styled::*;
use wasm_bindgen::JsCast;

use super::SHOWN;
use super::serving::fonts;

/// Where the root's canvases go: above the host's canvas, which shows only the background, and
/// passing every input through to it, which the root routes.
const LAYER: &str = "layers";

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

struct Canvas {
    /// The id the root shows it by.
    id: u32,
    element: web_sys::HtmlCanvasElement,
    style: String,
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
    shown: HashMap<Key, Canvas>,
    /// The screens of apps that could not be handed a canvas (an app that stopped), until their
    /// layers leave the list: none is made for them again each frame.
    refused: std::collections::HashSet<Key>,
    next: u32,
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
    /// leaves the page. Then draws the root's own surfaces into theirs.
    pub(super) fn show(
        &mut self,
        root: &Root,
        list: LayerList,
        context: &beui::native::Context,
    ) -> Result<(), String> {
        // CSS pixels a point, as be-host places its apps' screens.
        let scale = context.pixels_per_point() / context.native_pixels_per_point();
        let mut left = std::mem::take(&mut self.shown);
        let mut refused = std::collections::HashSet::new();
        let layers = list
            .windows
            .into_iter()
            .filter(|window| window.window.is_none())
            .flat_map(|window| window.layers)
            .filter(|layer| layer.shown);
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
            let mut canvas = match left.remove(&key) {
                Some(canvas) => canvas,
                None => match self.create(root, key) {
                    Ok(canvas) => canvas,
                    // An app that stopped, which the list names until the root hears so, shows
                    // nothing; the root's own surfaces must show.
                    Err(_) if matches!(key, Key::App { .. }) => {
                        refused.insert(key);
                        continue;
                    }
                    Err(why) => return Err(why),
                },
            };
            if canvas.style != style {
                let _ = canvas.element.set_attribute("style", &style);
                canvas.style = style;
            }
            self.shown.insert(key, canvas);
        }
        for (key, canvas) in left {
            forget(root, key, &canvas);
        }
        self.refused = refused;
        root.paint().map(|_| ())
    }

    /// A canvas in `#layers` for `key`, handed to whoever draws it.
    fn create(&mut self, root: &Root, key: Key) -> Result<Canvas, String> {
        let failed = |_| "a canvas for the root's frame could not be made".to_owned();
        let layer = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(LAYER))
            .ok_or_else(|| format!("the page has no #{LAYER} element for the root to show in"))?;
        let element: web_sys::HtmlCanvasElement = layer
            .owner_document()
            .ok_or("the page has no document")?
            .create_element("canvas")
            .map_err(failed)?
            .dyn_into()
            .map_err(|_| "a canvas for the root's frame could not be made".to_owned())?;
        // Which kind of canvas this is, for whatever inspects the page (crates/be-host/web-test):
        // an app's anchored view is an overlay, as be-host's own page names the canvas it draws
        // them in.
        let kind = match key {
            Key::Root(_) => "root",
            Key::App {
                anchored: false, ..
            } => "screen",
            Key::App { anchored: true, .. } => "overlay",
        };
        let _ = element.set_attribute("data-kind", kind);
        layer.append_child(&element).map_err(failed)?;
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        let handed = element
            .transfer_control_to_offscreen()
            .map_err(|_| "a canvas for the root's frame could not be handed over".to_owned())
            .and_then(|offscreen| match key {
                Key::Root(surface) => {
                    root.show_surface(id, &offscreen, surface);
                    Ok(())
                }
                Key::App {
                    instance, surface, ..
                } => root.show_app_surface(id, &offscreen, instance, surface),
            });
        if let Err(why) = handed {
            element.remove();
            return Err(why);
        }
        Ok(Canvas {
            id,
            element,
            style: String::new(),
        })
    }

    /// Takes every canvas off the page, as the root stops.
    pub(super) fn clear(&mut self, root: Option<&Root>) {
        for (key, canvas) in std::mem::take(&mut self.shown) {
            match root {
                Some(root) => forget(root, key, &canvas),
                None => canvas.element.remove(),
            }
        }
    }
}

/// Takes `canvas` off the page, and the root forgets it.
fn forget(root: &Root, key: Key, canvas: &Canvas) {
    match key {
        Key::Root(_) => root.forget_surface(canvas.id),
        Key::App { instance, .. } => root.forget_app_surface(canvas.id, instance),
    }
    canvas.element.remove();
}
