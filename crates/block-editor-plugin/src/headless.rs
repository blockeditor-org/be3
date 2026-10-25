use block_plugin_api::{
    EditorInstanceId, Message, ScreenLayout, ScreenPlacement, ops::guest::Executor,
};

use crate::{
    EditorHost, Frame, Instance, Waker,
    editor_session::EditorSession,
    screens::Screens,
    session::{ClientSession, State},
    wire::{Delivery, Shared},
};

pub struct HeadlessPlugin {
    session: ClientSession,
    screens: Screens,
    drawn: bool,
    wire: Shared,
    executor: Executor,
    frames: Vec<Vec<u8>>,
}

impl HeadlessPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        crate::clock::restart();
        let wire = Shared::default();
        let executor = wire.executor();
        executor.enter(|| wire.watch_shell());
        Self {
            session: ClientSession::new(id, name, version),
            screens: Screens::new(Vec::new(), Waker::default()),
            drawn: false,
            wire,
            executor,
            frames: Vec::new(),
        }
    }

    pub fn deliver(&mut self, frames: Vec<Vec<u8>>) -> Result<Vec<Message>, String> {
        self.executor.step(frames, &mut self.frames)?;
        let mut replies = Vec::new();
        for delivery in self.wire.take_inbox() {
            match delivery {
                Delivery::Message(message) => replies.extend(self.receive(message)),
                Delivery::Editor(instance, call) => {
                    let screens = &mut self.screens;
                    self.executor.enter(|| screens.call(instance, call));
                }
                Delivery::Screens(call) => call(&mut self.screens),
            }
        }
        Ok(replies)
    }

    pub fn take_frames(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.frames)
    }

    pub fn hello(&self) -> Message {
        self.session.hello()
    }

    pub fn adopt(&mut self, instance: EditorInstanceId, app: Box<dyn Instance>, host: EditorHost) {
        let screens = &mut self.screens;
        self.executor.enter(|| {
            screens.adopt(instance, EditorSession::adopt(instance, app, host));
        });
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        if matches!(self.session.state(), State::Closed | State::Failed) {
            return Vec::new();
        }
        let screens = &mut self.screens;
        self.executor.enter(|| screens.receive(&message));
        let painting = matches!(
            &message,
            Message::Extension { key, .. } if *key == paint_format::PAINTING_EXTENSION
        );
        let mut replies = self.session.receive(message);
        if painting {
            replies.push(self.screens.paintings());
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
            frames.push((
                *placement,
                self.executor.enter(|| session.run(placement.region, 0)),
            ));
            self.screens.ran(placement.instance);
        }
        self.drawn = true;
        frames
    }

    pub fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        if std::mem::take(&mut self.drawn) {
            messages.push(Message::Frame(self.screens.commit()));
        }
        let screens = &mut self.screens;
        messages.extend(self.executor.enter(|| screens.outbound()));
        if let Err(error) = self.executor.step(Vec::new(), &mut self.frames) {
            panic!("the headless plugin's executor failed: {error}");
        }
        messages
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

#[cfg(test)]
mod tests;
