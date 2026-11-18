//! Routing each window's raw input over the layers the root drew last.

use be_lib_input_router::{Routed, Router};
use be_lib_layers::{Layer, LayerId, LayerList, Surface, WindowLayers};
use be_lib_ui_types::input::{Input, KeyChord, Timed};

/// What the router delivers to one surface: `input`, its positions in the surface's own pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    pub target: LayerId,
    pub surface: Surface,
    pub input: Input,
    /// When the input it came of was sampled (`Timed::time`); none for what the router made
    /// itself, such as a focus moving.
    pub time: Option<u64>,
}

/// One window's routing: its router, and the layers it routes by.
struct Window {
    window: Option<u64>,
    router: Router<LayerId>,
    layers: WindowLayers,
    /// The last list drawn left it out: it went, and is forgotten once nothing holds it.
    gone: bool,
    /// The layer whose pointer lock the window's input took away (the platform released it,
    /// or the focus moved), which is not granted it again until it stops asking for it, so a
    /// lock the user let go of is not taken back at once from a request not yet withdrawn.
    unlocked: Option<LayerId>,
}

/// A router per window over the layers drawn last.
///
/// Input for a window is never dropped, even one the last list leaves out, since a release
/// dropped would leave its press held: a window the root never drew routes against no layers,
/// and one that went routes against the layers it had last until every press on it lifts. Then
/// its router goes, and the surface it had focused hears it lost the focus.
pub struct Windows {
    compositor: LayerId,
    /// The chords every window's router hands `compositor` ahead of the focus.
    intercepted: Vec<KeyChord>,
    windows: Vec<Window>,
}

impl Windows {
    /// Routes intercepted keys and system gestures to `compositor`.
    pub fn new(compositor: LayerId) -> Self {
        Self {
            compositor,
            intercepted: Vec::new(),
            windows: Vec::new(),
        }
    }

    /// Hands `chords` to the compositor in every window from now on, ahead of the layer focused
    /// there, in place of those it took before: a key pressed with one of them goes to the
    /// compositor, and so does its release. A dialog takes Escape this way while it is open.
    pub fn intercept(&mut self, chords: Vec<KeyChord>) {
        for routing in &mut self.windows {
            routing.router.intercept(chords.clone());
        }
        self.intercepted = chords;
    }

    /// Moves the keyboard in `window` to `target`, or takes it from every layer there: a dialog
    /// that opens takes it, and gives it back to the layer that had it once it closes. Answers
    /// what that tells the layer that had it and the one that gets it. A layer the window was not
    /// drawn with last cannot take it, so the keyboard goes to no layer: a layer is focused
    /// once it has been drawn, and the focus given back to one that went meanwhile is lost.
    pub fn focus(&mut self, window: Option<u64>, target: Option<LayerId>) -> Vec<Delivery> {
        let at = self.at(window);
        let routing = &mut self.windows[at];
        // A layer that went while the focus was elsewhere cannot take it: it goes nowhere.
        let target = target.filter(|target| routing.layers.layer(*target).is_some());
        let routed = routing.router.focus(target);
        delivered(&routing.layers, routed, None).collect()
    }

    /// Whether `window` has a router.
    pub fn routes(&self, window: Option<u64>) -> bool {
        self.windows.iter().any(|routing| routing.window == window)
    }

    /// The layer focused in `window`, as it was drawn last: the one keys go to.
    pub fn focused(&self, window: Option<u64>) -> Option<&Layer> {
        let routing = self.routing(window)?;
        routing.layers.layer(routing.router.focused()?)
    }

    /// The layer under the pointer in `window`, as it was drawn last: the one whose cursor
    /// shape the window shows.
    pub fn hovered(&self, window: Option<u64>) -> Option<&Layer> {
        let routing = self.routing(window)?;
        routing.layers.layer(routing.router.hovered()?)
    }

    /// The layer in `window` holding the pointer lock, which only the focused one may.
    pub fn locked(&self, window: Option<u64>) -> Option<&Layer> {
        let routing = self.routing(window)?;
        routing.layers.layer(routing.router.locked()?)
    }

    /// Grants the pointer lock in each window to its focused layer if `wants` says it asks for
    /// it, and releases it from a layer that no longer asks or lost the focus. A layer whose
    /// lock the window's input took away (the platform released it) is granted it again only
    /// once it stopped asking and asks anew. Answers what that tells the layer that lost it.
    pub fn lock(&mut self, wants: impl Fn(&Layer) -> bool) -> Vec<Delivery> {
        let mut deliveries = Vec::new();
        for routing in &mut self.windows {
            if let Some(unlocked) = routing.unlocked
                && !routing.layers.layer(unlocked).is_some_and(&wants)
            {
                routing.unlocked = None;
            }
            let asking = routing.router.focused().filter(|focused| {
                routing.unlocked != Some(*focused)
                    && routing.layers.layer(*focused).is_some_and(&wants)
            });
            if routing.router.locked().is_some() && routing.router.locked() != asking {
                let released = routing.router.unlock();
                deliveries.extend(delivered(&routing.layers, released, None));
            }
            if let Some(asking) = asking {
                routing.router.lock(asking);
            }
        }
        deliveries
    }

    /// The layers `window` was drawn with last, bottom first: none for a window never drawn.
    pub fn layers(&self, window: Option<u64>) -> &[Layer] {
        self.routing(window)
            .map_or(&[], |routing| &routing.layers.layers)
    }

    fn routing(&self, window: Option<u64>) -> Option<&Window> {
        self.windows.iter().find(|routing| routing.window == window)
    }

    /// Takes `list` as the layers drawn last. A window it leaves out has gone: its router goes
    /// once nothing is pressed on it. A focused layer a window's list leaves out loses the
    /// focus, which goes to no layer; a press it holds still ends there. Answers what that tells the surfaces those windows
    /// focused.
    pub fn drew(&mut self, list: LayerList) -> Vec<Delivery> {
        for routing in &mut self.windows {
            routing.gone = true;
        }
        for layers in list.windows {
            let window = layers.window.map(|window| window.raw());
            let router = self.router();
            match self
                .windows
                .iter_mut()
                .find(|routing| routing.window == window)
            {
                Some(routing) => {
                    routing.layers = layers;
                    routing.gone = false;
                    // A focused layer the list leaves out went: the keyboard goes to no layer,
                    // rather than to whatever later takes its id without hearing it was focused.
                    if let Some(focused) = routing.router.focused()
                        && routing.layers.layer(focused).is_none()
                    {
                        routing.router.focus(None);
                    }
                }
                None => self.windows.push(Window {
                    window,
                    router,
                    layers,
                    gone: false,
                    unlocked: None,
                }),
            }
        }
        self.prune()
    }

    /// Routes `window`'s `events` by its layers, and answers each delivery with the surface it
    /// is for and the time of the event it came of.
    pub fn route(&mut self, window: Option<u64>, events: Vec<Timed>) -> Vec<Delivery> {
        let at = self.at(window);
        let routing = &mut self.windows[at];
        let mut deliveries = Vec::new();
        for Timed { input, time } in events {
            let locked = routing.router.locked();
            let routed = routing.router.route(&routing.layers, input);
            if locked.is_some() && routing.router.locked().is_none() {
                routing.unlocked = locked;
            }
            deliveries.extend(delivered(&routing.layers, routed, time));
        }
        deliveries.extend(self.prune());
        deliveries
    }

    /// Where `window`'s routing is, made for it if it has none: one never drawn routes against
    /// no layers, and is forgotten once nothing holds it.
    fn at(&mut self, window: Option<u64>) -> usize {
        if let Some(at) = self
            .windows
            .iter()
            .position(|routing| routing.window == window)
        {
            return at;
        }
        let router = self.router();
        self.windows.push(Window {
            window,
            router,
            layers: WindowLayers {
                window: None,
                layers: Vec::new(),
            },
            gone: true,
            unlocked: None,
        });
        self.windows.len() - 1
    }

    /// A new window's router, which hands the compositor the chords it intercepts.
    fn router(&self) -> Router<LayerId> {
        let mut router = Router::new(self.compositor);
        router.intercept(self.intercepted.clone());
        router
    }

    /// Forgets the windows that went and hold no press, and answers the focus each lost.
    fn prune(&mut self) -> Vec<Delivery> {
        let mut lost = Vec::new();
        self.windows.retain(|routing| {
            let keep = !routing.gone || routing.router.holding();
            if !keep
                && let Some(target) = routing.router.focused()
                && let Some(layer) = routing
                    .layers
                    .layers
                    .iter()
                    .find(|layer| layer.id == target)
            {
                lost.push(Delivery {
                    target,
                    surface: layer.surface,
                    input: Input::Focus(false),
                    time: None,
                });
            }
            keep
        });
        lost
    }
}

/// What `routed` delivers to the layers of `layers`, each at `time`.
fn delivered(
    layers: &WindowLayers,
    routed: Vec<Routed<LayerId>>,
    time: Option<u64>,
) -> impl Iterator<Item = Delivery> + '_ {
    routed.into_iter().filter_map(move |routed| {
        // The root starts no drag between surfaces, so only input is routed.
        let Routed::Input { target, input } = routed else {
            return None;
        };
        let layer = layers.layer(target)?;
        Some(Delivery {
            target,
            surface: layer.surface,
            input,
            time,
        })
    })
}

#[cfg(test)]
mod tests;
