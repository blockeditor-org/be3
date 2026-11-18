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
//! - `Embedding` holds both: what is routed to an app's screen reaches the app as
//!   `AppCall::Input`, in the screen's points; what lands on the root's own surfaces the root
//!   gets back.
//!
//! The root stays single-threaded and never blocks: each app's frames are heard by a task on
//! the root's executor (`be_wasm_ops::guest::spawn`), and a frame awaits the present as ordinary
//! async code, so the root races it against its deadline (`be3:root`'s `host.deadline`).
//!
//! A root links it as any guest links a be-sdk crate: be-wasm crates may depend only on be-wasm
//! ones (buck/cargo/layers.bzl), but a root is not one. A root that links this is an app's
//! module (be-app, for be-host's own root) or, for tests, a be-sdk crate's
//! (be-sdk-root-testing); both families may link be-sdk.

mod embedded;
mod windows;

pub use embedded::{Embedded, Spawn};
pub use windows::{Delivery, Windows};

use std::time::Duration;

use be_lib_layers::{LayerId, LayerList, Surface};
use be_lib_ui_types::input;

/// The apps a root embeds and the routing of its windows' input to them.
pub struct Embedding {
    apps: Vec<Embedded>,
    windows: Windows,
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

    /// The frame clock's time the root wants its next frame drawn for, for its apps: the
    /// earliest any of them wants (`Embedded::next_frame`), as `be3:root`'s `frame-done` carries
    /// it.
    pub fn next_frame(&self) -> Option<Duration> {
        self.apps.iter().filter_map(Embedded::next_frame).min()
    }

    /// Routes `window`'s input (`events`, be-lib-ui-types' `input::encode`, as `be3:root`'s
    /// `input.deliver` carries it) over the layers drawn last: what lands on an app's screen goes
    /// to the app; what lands on the root's own surfaces is answered, for the root to take in.
    pub fn input(&mut self, window: Option<u64>, events: &[u8]) -> Result<Vec<Delivery>, String> {
        let events = input::decode(events)?;
        let routed = self.windows.route(window, events);
        Ok(self.forward(routed))
    }

    /// Takes `list` as the layers drawn last, which input is routed by from now on, and answers
    /// it as `be3:root`'s `frame-done` carries it (`LayerList::encode`). An app focused in a
    /// window the list leaves out hears it lost the focus; the root's own surfaces are answered
    /// what they hear of it.
    pub fn drew(&mut self, list: LayerList) -> Drawn {
        let layers = list.encode();
        let lost = self.windows.drew(list);
        Drawn {
            layers,
            own: self.forward(lost),
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
                app.input(surface, delivery.input);
            }
        }
        own
    }
}

/// A frame the root drew, as `Embedding::drew` takes it.
pub struct Drawn {
    /// The layer list, as `be3:root`'s `frame-done` carries it.
    pub layers: Vec<u8>,
    /// What the root's own surfaces hear of it: the focus a window that went took with it.
    pub own: Vec<Delivery>,
}
