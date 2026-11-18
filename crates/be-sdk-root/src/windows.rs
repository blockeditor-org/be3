//! Routing each window's raw input over the layers the root drew last.

use be_lib_input_router::{Routed, Router};
use be_lib_layers::{LayerId, LayerList, Surface, WindowLayers};
use be_lib_ui_types::input::Input;

/// What the router delivers to one surface: `input`, its positions in the surface's own pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    pub target: LayerId,
    pub surface: Surface,
    pub input: Input,
}

/// One window's routing: its router, and the layers it routes by.
struct Window {
    window: Option<u64>,
    router: Router<LayerId>,
    layers: WindowLayers,
    /// The last list drawn left it out: it went, and is forgotten once nothing holds it.
    gone: bool,
}

/// A router per window over the layers drawn last.
///
/// Input for a window is never dropped, even one the last list leaves out, since a release
/// dropped would leave its press held: a window the root never drew routes against no layers,
/// and one that went routes against the layers it had last until every press on it lifts. Then
/// its router goes, and the surface it had focused hears it lost the focus.
pub struct Windows {
    compositor: LayerId,
    windows: Vec<Window>,
}

impl Windows {
    /// Routes intercepted keys and system gestures to `compositor`.
    pub fn new(compositor: LayerId) -> Self {
        Self {
            compositor,
            windows: Vec::new(),
        }
    }

    /// Whether `window` has a router.
    pub fn routes(&self, window: Option<u64>) -> bool {
        self.windows.iter().any(|routing| routing.window == window)
    }

    /// Takes `list` as the layers drawn last. A window it leaves out has gone: its router goes
    /// once nothing is pressed on it. Answers what that tells the surfaces those windows
    /// focused.
    pub fn drew(&mut self, list: LayerList) -> Vec<Delivery> {
        for routing in &mut self.windows {
            routing.gone = true;
        }
        for layers in list.windows {
            let window = layers.window.map(|window| window.raw());
            match self
                .windows
                .iter_mut()
                .find(|routing| routing.window == window)
            {
                Some(routing) => {
                    routing.layers = layers;
                    routing.gone = false;
                }
                None => self.windows.push(Window {
                    window,
                    router: Router::new(self.compositor),
                    layers,
                    gone: false,
                }),
            }
        }
        self.prune()
    }

    /// Routes `window`'s `events` by its layers, and answers each delivery with the surface it
    /// is for.
    pub fn route(&mut self, window: Option<u64>, events: Vec<Input>) -> Vec<Delivery> {
        let at = match self
            .windows
            .iter()
            .position(|routing| routing.window == window)
        {
            Some(at) => at,
            None => {
                self.windows.push(Window {
                    window,
                    router: Router::new(self.compositor),
                    layers: WindowLayers {
                        window: None,
                        layers: Vec::new(),
                    },
                    gone: true,
                });
                self.windows.len() - 1
            }
        };
        let routing = &mut self.windows[at];
        let mut deliveries = Vec::new();
        for event in events {
            for routed in routing.router.route(&routing.layers, event) {
                // The root starts no drag between surfaces, so only input is routed.
                let Routed::Input { target, input } = routed else {
                    continue;
                };
                let Some(layer) = routing
                    .layers
                    .layers
                    .iter()
                    .find(|layer| layer.id == target)
                else {
                    continue;
                };
                deliveries.push(Delivery {
                    target,
                    surface: layer.surface,
                    input,
                });
            }
        }
        deliveries.extend(self.prune());
        deliveries
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
                });
            }
            keep
        });
        lost
    }
}

#[cfg(test)]
mod tests;
