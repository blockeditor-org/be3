use block_plugin_api::{
    Direction, EditorInstanceId, EditorMessage, ErrorCode, Hello, Message, PROTOCOL_VERSION,
    PluginIdentity, ProtocolError, ScreenId, SurfaceSpec, SurfaceSupport,
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    AwaitingHello,
    Running,
    Closed,
    Failed,
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

    pub fn hello(&self) -> Message {
        Message::Hello(Hello {
            version: PROTOCOL_VERSION,
            plugin: self.plugin.clone(),
            surface: match cfg!(target_arch = "wasm32") {
                true => SurfaceSupport::Texture,
                false => SurfaceSupport::None,
            },
        })
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        match self.dispatch(message) {
            Ok(messages) => messages,
            Err(message) => {
                self.state = State::Failed;
                vec![Message::Error(ProtocolError {
                    request_id: None,
                    code: ErrorCode::InvalidState,
                    message,
                })]
            }
        }
    }

    fn dispatch(&mut self, message: Message) -> Result<Vec<Message>, String> {
        if matches!(self.state, State::Closed | State::Failed) {
            return Ok(Vec::new());
        }
        if message.direction() == Direction::ToHost {
            return Err(format!(
                "the host sent {}, which only a plugin may send",
                name(&message)
            ));
        }
        match (self.state, message) {
            (State::AwaitingHello, Message::HelloAccepted(accepted)) => {
                if accepted.version != PROTOCOL_VERSION {
                    return Err(format!(
                        "the host speaks protocol version {} and this plugin speaks {PROTOCOL_VERSION}",
                        accepted.version
                    ));
                }
                self.surface = accepted.surface;
                self.state = State::Running;
                Ok(Vec::new())
            }
            (State::AwaitingHello, Message::HelloRejected(error)) => Err(error.message),
            (State::AwaitingHello, message) => Err(format!(
                "the host sent {} before accepting this plugin",
                name(&message)
            )),
            (State::Running, Message::Screens(set)) => {
                if self.surface.is_none() {
                    return Err("the host sent screens to a plugin it gave no surface".into());
                }
                if let Some(request) = set
                    .screens
                    .iter()
                    .find(|screen| !self.instances.contains(&screen.instance))
                {
                    return Err(format!(
                        "a screen referenced editor instance {}, which is not open",
                        request.instance.0
                    ));
                }
                self.screens = set.screens.iter().map(|screen| screen.screen).collect();
                Ok(vec![Message::Acknowledged {
                    request_id: set.request_id,
                }])
            }
            (State::Running, Message::Input(input)) => match self.screens.contains(&input.screen) {
                true => Ok(Vec::new()),
                false => Err(format!(
                    "input arrived for screen {}, which this plugin was not given",
                    input.screen.0
                )),
            },
            (State::Running, Message::ChildStatuses(statuses)) => {
                match statuses
                    .iter()
                    .all(|status| self.instances.contains(&status.instance))
                {
                    true => Ok(Vec::new()),
                    false => Err("a child status named an editor instance that is not open".into()),
                }
            }
            (State::Running, Message::Editor(editor)) => self.editor(editor),
            (State::Running, Message::DrawFrame) => match self.surface.is_some() {
                true => Ok(Vec::new()),
                false => Err("the host asked a plugin it gave no surface to draw a frame".into()),
            },
            (State::Running, Message::Shutdown) => {
                self.state = State::Closed;
                Ok(vec![Message::ShutdownAcknowledged])
            }
            (State::Running, _) => Ok(Vec::new()),
            (State::Closed | State::Failed, _) => Ok(Vec::new()),
        }
    }

    fn editor(&mut self, message: EditorMessage) -> Result<Vec<Message>, String> {
        let instance = message.instance();
        match message {
            EditorMessage::Open { .. }
            | EditorMessage::OpenCreation { .. }
            | EditorMessage::OpenArtifact { .. } => match self.instances.insert(instance) {
                true => Ok(Vec::new()),
                false => Err(format!("editor instance {} was opened twice", instance.0)),
            },
            EditorMessage::Close { .. } => match self.instances.remove(&instance) {
                true => Ok(Vec::new()),
                false => Err(format!(
                    "editor instance {} was closed without being open",
                    instance.0
                )),
            },
            _ => match self.instances.contains(&instance) {
                true => Ok(Vec::new()),
                false => Err(format!(
                    "a message named editor instance {}, which is not open",
                    instance.0
                )),
            },
        }
    }
}

fn name(message: &Message) -> &'static str {
    match message {
        Message::Hello(_) => "a hello",
        Message::HelloAccepted(_) => "an accepted hello",
        Message::HelloRejected(_) => "a rejected hello",
        Message::Theme(_) => "a theme",
        Message::Screens(_) => "a screen set",
        Message::Layout(_) => "a layout",
        Message::RegionSizes(_) => "region sizes",
        Message::Frames(_) => "frame reports",
        Message::Input(_) => "input",
        Message::DrawFrame => "a draw request",
        Message::FrameNeeded => "a frame request",
        Message::FrameReady(_) => "a ready frame",
        Message::Acknowledged { .. } => "an acknowledgement",
        Message::Error(_) => "an error",
        Message::Shutdown => "a shutdown",
        Message::ShutdownAcknowledged => "a shutdown acknowledgement",
        Message::Editor(_) => "an editor message",
        Message::BlockTypes(_) => "block types",
        Message::Children(_) => "child placements",
        Message::ChildStatuses(_) => "child statuses",
    }
}

#[cfg(test)]
mod tests;
