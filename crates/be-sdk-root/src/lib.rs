//! What a root guest links to embed apps (plans/host_as_platform_adapter.md, The root guest):
//! the host's half of the app protocol (be-sdk-app-host) run inside the root, over the channel
//! of each app it spawns with the sandbox (`be3:sandbox`), with no GUI framework.
//!
//! - `Embedded` is one app the root spawned: a `Wire` over the instance's channel and a `Run`
//!   with its session, pacing and holds. It opens the editor and its screens once the app says
//!   hello, asks it for a frame when its pacing says one is due and waits for the present, and
//!   names each screen it painted by handle as a layer (`Embedded::layer`), whose pixels stay on
//!   the host's device.
//! - `Windows` routes each window's raw input with be-lib-input-router's `Router` over the
//!   layers the root drew last, as the host's own UI routes today.
//! - `OwnSurface` is a surface of the root's own, which it draws on the host's device as an app
//!   draws its screens and names in its layer list as `Surface::Root`, below, between or above
//!   the screens it embeds.
//! - `Embedded::spawn_block` embeds a block's editor its host opens and serves until step 5
//!   (decisions/a_roots_embedded_editors_are_served_by_its_host_until_step_5.md): `be3:root`'s
//!   `host.editor-of` names the app, `host.open-editor` opens the editor, and everything the app
//!   sends under it the root forwards (`host.forward`) and relays what the host answers
//!   (`host.forwarded`), as be-wasm-ops frames it never reads.
//! - `Children` shows the block children the editors of an embedded app place: each is its
//!   block's editor in an app of its own, spawned as `spawn_block` does and composited where it
//!   was placed, with its own children likewise; the host tells each placer how they are
//!   (`host.show-children`).
//! - `Embedding` holds the `Embedded`s and `Windows`: what is routed to an app's screen reaches
//!   the app as `AppCall::Input`, in the screen's points; what lands on the root's own surfaces
//!   the root gets back. What its apps ask of their windows (a cursor shape, the pointer lock,
//!   every sample) it answers as the window's (`Embedding::requests`).
//!
//! The clipboard is not relayed: an app it embeds copies and pastes only as the app protocol
//! lets it (a copy as a host service, which this does not serve yet, and a paste as the input
//! its host delivers), never through the root's own `be3:root` `host.copy` and `host.paste`,
//! which the host trusts the root alone with. A root never hands an app those calls or their
//! answers.
//!
//! The root stays single-threaded and never blocks: each app's frames are heard by a task on
//! the root's executor (`be_wasm_ops::guest::spawn`), and a frame awaits the present as ordinary
//! async code, so the root races it against its deadline (`be3:root`'s `host.deadline`).
//!
//! A root links it as any guest links a be-sdk crate: be-wasm crates may depend only on be-wasm
//! ones (buck/cargo/layers.bzl), but a root is not one. A root that links this is an app's
//! module (be-app, for be-host's own root) or, for tests, a be-sdk crate's
//! (be-sdk-root-testing); both families may link be-sdk. A root draws its own surfaces with
//! beui through be-sdk-beui-root, beside this.

mod children;
mod embedded;
mod own;
mod windows;

pub use children::{
    ALREADY_OPEN_ABOVE, Children, KeyboardMove, NOT_SHOWN_YET, Spawning, child_screen,
    framing_screen, main_screen, preview_screen, preview_screen_of,
};
pub use embedded::{BlockSpawn, Embedded, Hosted, Spawn, SpawnFailure, Wants};
pub use own::OwnSurface;
pub use windows::{Delivery, Windows};

use be_lib_geometry::{Rect, pos2};
use be_lib_layers::{Layer, LayerId, LayerList, Surface};
use be_lib_ui_types::{CursorIcon, ImeArea, KeyChord, WindowRequests, input};

/// The apps a root embeds and the routing of its windows' input to them.
pub struct Embedding {
    apps: Vec<Embedded>,
    windows: Windows,
}

/// What a window's input did (`Embedding::input`).
#[derive(Debug, Default)]
pub struct Routed {
    /// What the root's own surfaces hear of it.
    pub own: Vec<Delivery>,
    /// The presses and Escapes in it, in order, each with the layer it reached (an app's screen
    /// or the root's own): what takes back active children (`Children::revoke`).
    pub pressed: Vec<Pressed>,
}

/// A press, of a pointer, a finger or a pen, or Escape pressed, and the layer it reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pressed {
    pub layer: LayerId,
    /// Escape pressed, not a pointer's press.
    pub escape: bool,
}

impl Embedding {
    /// Routes the keys the root intercepts and the system's gestures to `compositor`, the
    /// root's own layer.
    pub fn new(compositor: LayerId) -> Self {
        Self {
            apps: Vec::new(),
            windows: Windows::new(compositor),
        }
    }

    /// Embeds `app`: input routed to its screens reaches it.
    pub fn embed(&mut self, app: Embedded) {
        self.apps.push(app);
    }

    /// Lets go of the app with the handle `instance`: once no other `Embedded` of it is left,
    /// its instance stops.
    pub fn remove(&mut self, instance: u64) {
        self.apps.retain(|app| app.instance() != instance);
    }

    /// The apps embedded, in the order they were.
    pub fn apps(&self) -> &[Embedded] {
        &self.apps
    }

    /// What the root's apps want of its frames (`Embedded::wants`): the earliest next frame
    /// any of them wants, and whether any owes the root something now, as `be3:root`'s
    /// `frame-done` carries them (`next`, `owed`).
    pub fn wants(&self) -> Wants {
        self.apps
            .iter()
            .map(Embedded::wants)
            .fold(Wants::default(), |all, app| Wants {
                next: match (all.next, app.next) {
                    (Some(left), Some(right)) => Some(left.min(right)),
                    (left, right) => left.or(right),
                },
                owed: all.owed || app.owed,
            })
    }

    /// Routes `window`'s input (`events`, be-lib-ui-types' `input::encode`, as `be3:root`'s
    /// `input.deliver` carries it) over the layers drawn last: what lands on an app's screen goes
    /// to the app, at the time it was sampled; what lands on the root's own surfaces is
    /// answered, for the root to take in.
    pub fn input(&mut self, window: Option<u64>, events: &[u8]) -> Result<Routed, String> {
        let events = input::decode(events)?;
        let routed = self.windows.route(window, events);
        let pressed = routed
            .iter()
            .filter_map(|delivery| {
                Some(Pressed {
                    layer: delivery.target,
                    escape: presses(&delivery.input)?,
                })
            })
            .collect();
        Ok(Routed {
            own: self.forward(routed),
            pressed,
        })
    }

    /// Hands `chords` to the root's own layer (the compositor `new` names) in every window, ahead
    /// of the layer focused there, in place of those it took before (`Windows::intercept`): the
    /// root's own shortcuts, and Escape while a dialog of its own is open.
    pub fn intercept(&mut self, chords: Vec<KeyChord>) {
        self.windows.intercept(chords);
    }

    /// The layer focused in `window`, as it was drawn last: the one its keys, text and input
    /// method go to.
    pub fn focused(&self, window: Option<u64>) -> Option<&Layer> {
        self.windows.focused(window)
    }

    /// Moves the keyboard in `window` to the layer `target`, or takes it from every layer there
    /// (`Windows::focus`): a dialog of the root's own takes it once drawn, and gives it back to
    /// the layer that had it once it closes. An app whose screen lost or got it hears so; what
    /// the root's own surfaces hear of it is answered.
    pub fn focus(&mut self, window: Option<u64>, target: Option<LayerId>) -> Vec<Delivery> {
        let told = self.windows.focus(window, target);
        self.forward(told)
    }

    /// The input method's area in `window`, in its points: the field the app focused there
    /// edits, as its screen asked, placed where the layers drawn last show that screen. None
    /// while the root's own surface has the focus, or the focused app edits no text.
    pub fn ime(&self, window: Option<u64>) -> Option<ImeArea> {
        let layer = self.windows.focused(window)?;
        let Surface::Screen { instance, surface } = layer.surface else {
            return None;
        };
        let app = self.apps.iter().find(|app| app.instance() == instance)?;
        let (area, raster) = app.ime(surface)?;
        Some(ime_in_window(area, raster, layer))
    }

    /// What `window` asks of its host for the apps it shows, as `be3:root`'s `frame-done`
    /// carries it: the cursor shape of the screen under the pointer, the pointer lock while the
    /// focused screen holds it (`drew` grants it), and every sample of the pointer's moves while
    /// a screen it shows asks for them. Over the root's own surfaces the cursor is the default
    /// one: a root that draws its own controls sets its own.
    pub fn requests(&self, window: Option<u64>) -> Requests {
        requests(&self.windows, window, |surface| {
            let Surface::Screen { instance, surface } = *surface else {
                return None;
            };
            let app = self.apps.iter().find(|app| app.instance() == instance)?;
            app.window(surface)
        })
    }

    /// Takes `list` as the layers drawn last, which input is routed by from now on, and answers
    /// it as `be3:root`'s `frame-done` carries it (`LayerList::encode`). The focused screen in
    /// each window gets the pointer lock if it asks for it, and one that stopped asking or lost
    /// the focus loses it. An app focused in a window the list leaves out hears it lost the
    /// focus; the root's own surfaces are answered what they hear of these.
    pub fn drew(&mut self, list: LayerList) -> Drawn {
        let layers = list.encode();
        let mut told = self.windows.drew(list);
        let apps = &self.apps;
        told.extend(self.windows.lock(|layer| {
            let Surface::Screen { instance, surface } = layer.surface else {
                return false;
            };
            apps.iter()
                .find(|app| app.instance() == instance)
                .and_then(|app| app.window(surface))
                .is_some_and(|window| window.pointer_locked)
        }));
        Drawn {
            layers,
            own: self.forward(told),
        }
    }

    /// Sends each app what is for its screens, and answers the rest.
    fn forward(&self, deliveries: Vec<Delivery>) -> Vec<Delivery> {
        let mut own = Vec::new();
        for delivery in deliveries {
            let Surface::Screen { instance, surface } = delivery.surface else {
                own.push(delivery);
                continue;
            };
            if let Some(app) = self.apps.iter().find(|app| app.instance() == instance) {
                app.input(surface, delivery.input, delivery.time);
            }
        }
        own
    }
}

/// Whether `input` is a press, of a pointer, a finger or a pen (`Some(false)`), or Escape
/// pressed (`Some(true)`): what takes an active child back from its placer.
fn presses(input: &input::Input) -> Option<bool> {
    use input::{Input, Key, PenPhase, TouchPhase};
    match input {
        Input::PointerButton { pressed: true, .. } => Some(false),
        Input::Touch {
            phase: TouchPhase::Start,
            ..
        } => Some(false),
        Input::Pen {
            phase: PenPhase::Down(_),
            ..
        } => Some(false),
        Input::Key(stroke)
            if stroke.pressed && !stroke.repeat && stroke.key == Some(Key::Escape) =>
        {
            Some(true)
        }
        _ => None,
    }
}

/// What a window asks of its host for the surfaces it shows (`Embedding::requests`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requests {
    /// The cursor shape the window shows.
    pub cursor: CursorIcon,
    /// Whether the pointer is locked to the window.
    pub pointer_locked: bool,
    /// Whether the window hands the root every sample of the pointer's moves, rather than
    /// merging a frame's into its last.
    pub every_sample: bool,
}

/// What `window` asks for its layers, given what the surface each shows asks of its window
/// (`asks`; none for one that asks nothing, such as the root's own).
pub(crate) fn requests(
    windows: &Windows,
    window: Option<u64>,
    asks: impl Fn(&Surface) -> Option<WindowRequests>,
) -> Requests {
    let cursor = windows
        .hovered(window)
        .and_then(|layer| asks(&layer.surface))
        .map_or(CursorIcon::Default, |asked| asked.cursor);
    let pointer_locked = windows
        .locked(window)
        .is_some_and(|layer| asks(&layer.surface).is_some_and(|asked| asked.pointer_locked));
    let every_sample = windows
        .layers(window)
        .iter()
        .filter(|layer| layer.shown)
        .any(|layer| asks(&layer.surface).is_some_and(|asked| asked.every_sample));
    Requests {
        cursor,
        pointer_locked,
        every_sample,
    }
}

/// `area`, an input method's area in a surface's points, in the window's points: the surface
/// is painted at `raster` pixels per point and shown by `layer`. An app's screen's area
/// (`Embedding::ime`), or a field in a document of the root's own.
pub fn ime_in_window(area: ImeArea, raster: f32, layer: &Layer) -> ImeArea {
    let pixels = |rect: Rect| {
        layer.to_window(Rect::from_min_max(
            pos2(rect.min.x * raster, rect.min.y * raster),
            pos2(rect.max.x * raster, rect.max.y * raster),
        ))
    };
    ImeArea {
        rect: pixels(area.rect),
        cursor: pixels(area.cursor),
        ..area
    }
}

/// A frame the root drew, as `Embedding::drew` takes it.
pub struct Drawn {
    /// The layer list, as `be3:root`'s `frame-done` carries it.
    pub layers: Vec<u8>,
    /// What the root's own surfaces hear of it: the focus a window that went took with it.
    pub own: Vec<Delivery>,
}

#[cfg(test)]
mod tests;
