use block_plugin_api::{
    EditorInstanceId, Hello, HelloAccepted, PluginIdentity, ScreenId, SurfaceSpec, SurfaceSupport,
};
use std::collections::HashSet;

use crate::wire::Delivery;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    AwaitingHello,
    Running,
    Closed,
}

pub struct ClientSession {
    state: State,
    screens: HashSet<ScreenId>,
    plugin: PluginIdentity,
    instances: HashSet<EditorInstanceId>,
    surface: Option<SurfaceSpec>,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

impl ClientSession {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            state: State::AwaitingHello,
            screens: HashSet::new(),
            plugin: PluginIdentity {
                id: id.into(),
                name: name.into(),
                version: version.into(),
            },
            instances: HashSet::new(),
            surface: None,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn surface(&self) -> Option<SurfaceSpec> {
        self.surface
    }

    pub fn hello(&self) -> Hello {
        Hello {
            plugin: self.plugin.clone(),
            surface: match cfg!(target_arch = "wasm32") {
                true => SurfaceSupport::Texture,
                false => SurfaceSupport::None,
            },
        }
    }

    pub(crate) fn receive(&mut self, delivery: &Delivery) -> Result<(), String> {
        match (self.state, delivery) {
            (State::Closed, _) => Ok(()),
            (State::AwaitingHello, Delivery::Accepted(accepted)) => {
                self.accept(accepted);
                Ok(())
            }
            (State::AwaitingHello, delivery) => Err(format!(
                "the host sent {} before accepting this plugin",
                delivery.name()
            )),
            (State::Running, Delivery::Accepted(_)) => {
                Err("the host accepted this plugin twice".into())
            }
            (State::Running, Delivery::Screens(screens)) => {
                if self.surface.is_none() {
                    return Err("the host sent screens to a plugin it gave no surface".into());
                }
                if let Some(request) = screens
                    .iter()
                    .find(|screen| !self.instances.contains(&screen.instance))
                {
                    return Err(format!(
                        "a screen referenced editor instance {}, which is not open",
                        request.instance.0
                    ));
                }
                self.screens = screens.iter().map(|screen| screen.screen).collect();
                Ok(())
            }
            (State::Running, Delivery::Input(input)) => {
                match input.screen.is_opened() || self.screens.contains(&input.screen) {
                    true => Ok(()),
                    false => Err(format!(
                        "input arrived for screen {}, which this plugin was not given",
                        input.screen.0
                    )),
                }
            }
            (State::Running, Delivery::Open(opening)) => {
                let instance = opening.instance();
                match self.instances.insert(instance) {
                    true => Ok(()),
                    false => Err(format!("editor instance {} was opened twice", instance.0)),
                }
            }
            (State::Running, Delivery::Close(instance)) => match self.instances.remove(instance) {
                true => Ok(()),
                false => Err(format!(
                    "editor instance {} was closed without being open",
                    instance.0
                )),
            },
            (State::Running, Delivery::Draw { .. }) => match self.surface.is_some() {
                true => Ok(()),
                false => Err("the host asked a plugin it gave no surface to draw a frame".into()),
            },
            (State::Running, Delivery::Shutdown) => {
                self.state = State::Closed;
                Ok(())
            }
            (State::Running, _) => Ok(()),
        }
    }

    fn accept(&mut self, accepted: &HelloAccepted) {
        self.surface = accepted.surface;
        self.state = State::Running;
    }
}

#[cfg(test)]
mod tests;
