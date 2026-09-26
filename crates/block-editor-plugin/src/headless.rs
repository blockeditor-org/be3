use block_plugin_api::{EditorInstanceId, Message, ScreenLayout, ScreenPlacement};

use crate::{
    EditorHost, Frame, Instance, Waker,
    editor_session::EditorSession,
    screens::Screens,
    session::{ClientSession, State},
};

pub struct HeadlessPlugin {
    session: ClientSession,
    screens: Screens,
    generation: u64,
}

impl HeadlessPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new(Vec::new(), Waker::default()),
            generation: 0,
        }
    }

    pub fn hello(&self) -> Message {
        self.session.hello()
    }

    pub fn adopt(&mut self, instance: EditorInstanceId, app: Box<dyn Instance>, host: EditorHost) {
        self.screens
            .adopt(instance, EditorSession::adopt(instance, app, host));
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        if matches!(self.session.state(), State::Closed | State::Failed) {
            return Vec::new();
        }
        let before = self.screens.layout().clone();
        self.screens.receive(&message);
        let replies = self.session.receive(message);
        let layout = self.screens.layout();
        if !layout.is_empty() && !layout.same_placements(&before) {
            self.generation += 1;
            self.screens.set_generation(self.generation);
        }
        replies
    }

    pub fn draw(&mut self) -> Vec<(ScreenPlacement, Frame)> {
        let layout = self.screens.layout().clone();
        let mut frames = Vec::new();
        for placement in &layout.screens {
            let Some(session) = self.screens.session(placement.instance) else {
                continue;
            };
            frames.push((*placement, session.run(placement.region, layout.generation)));
        }
        frames
    }

    pub fn outbound(&mut self) -> Vec<Message> {
        self.screens.outbound()
    }

    pub fn layout(&self) -> &ScreenLayout {
        self.screens.layout()
    }

    pub fn host(&self, instance: EditorInstanceId) -> Option<EditorHost> {
        Some(self.screens.get(instance)?.host().clone())
    }

    pub fn instance(&self, instance: EditorInstanceId) -> Option<&dyn Instance> {
        Some(self.screens.get(instance)?.instance())
    }

    pub fn instance_mut(&mut self, instance: EditorInstanceId) -> Option<&mut dyn Instance> {
        Some(self.screens.session(instance)?.instance_mut())
    }
}
