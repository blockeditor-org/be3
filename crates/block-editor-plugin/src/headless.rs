use block_plugin_api::{
    EditorInstanceId, Fonts, FrameCommit, Hello, HelloAccepted, InputBatch, Opening, Preferences,
    ScreenLayout, ScreenPlacement, ScreenRequest, ops::guest::Executor,
};

use crate::{
    EditorHost, Frame, Instance, Module, Waker,
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

    pub fn deliver(&mut self, frames: Vec<Vec<u8>>) -> Result<(), String> {
        self.executor.step(frames, &mut self.frames)?;
        for delivery in self.wire.take_inbox() {
            self.receive(delivery)?;
        }
        Ok(())
    }

    pub fn take_frames(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.frames)
    }

    pub fn hello(&self) -> Hello {
        self.session.hello()
    }

    pub fn adopt(&mut self, instance: EditorInstanceId, app: Box<dyn Instance>, host: EditorHost) {
        let screens = &mut self.screens;
        self.executor.enter(|| {
            screens.adopt(instance, EditorSession::adopt(instance, app, host));
        });
    }

    pub fn accept(&mut self, accepted: HelloAccepted) -> Result<(), String> {
        self.receive(Delivery::Accepted(accepted))
    }

    pub fn set_preferences(&mut self, preferences: Preferences) -> Result<(), String> {
        self.receive(Delivery::Preferences(preferences))
    }

    pub fn set_fonts(&mut self, fonts: Fonts) -> Result<(), String> {
        self.receive(Delivery::Fonts(fonts))
    }

    pub fn screens(&mut self, screens: Vec<ScreenRequest>) -> Result<(), String> {
        self.receive(Delivery::Screens(screens))
    }

    pub fn configure(&mut self, serial: u64) -> Result<(), String> {
        self.receive(Delivery::Configure(serial))
    }

    pub fn input(&mut self, batch: InputBatch) -> Result<(), String> {
        self.receive(Delivery::Input(batch))
    }

    pub fn draw_frame(&mut self, now_micros: u64, seen: bool) -> Result<(), String> {
        self.receive(Delivery::Draw { now_micros, seen })
    }

    pub fn open(&mut self, opening: Opening) -> Result<(), String> {
        self.receive(Delivery::Open(opening))
    }

    pub fn close(&mut self, instance: EditorInstanceId) -> Result<(), String> {
        self.receive(Delivery::Close(instance))
    }

    fn receive(&mut self, delivery: Delivery) -> Result<(), String> {
        if self.session.state() == State::Closed {
            return Ok(());
        }
        self.session.receive(&delivery)?;
        match delivery {
            Delivery::Paintings(painted) => painted.send(self.screens.paintings()),
            Delivery::Shutdown => self.wire.shut_down(),
            delivery => {
                let screens = &mut self.screens;
                self.executor.enter(|| screens.deliver(delivery));
            }
        }
        Ok(())
    }

    pub fn draw(&mut self) -> Vec<(ScreenPlacement, Frame)> {
        let layout = self.screens.layout().clone();
        let shown: Vec<(ScreenPlacement, u32)> = layout
            .screens
            .iter()
            .map(|placement| (*placement, 0))
            .collect();
        let screens = &mut self.screens;
        let frames = self.executor.enter(|| screens.frame(&shown));
        self.drawn = true;
        layout
            .screens
            .into_iter()
            .zip(frames)
            .filter(|(placement, _)| self.screens.get(placement.instance).is_some())
            .collect()
    }

    pub fn outbound(&mut self) -> Option<FrameCommit> {
        let commit = std::mem::take(&mut self.drawn).then(|| self.screens.commit());
        let screens = &mut self.screens;
        self.executor.enter(|| screens.outbound());
        if let Err(error) = self.executor.step(Vec::new(), &mut self.frames) {
            panic!("the headless plugin's executor failed: {error}");
        }
        commit
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

    pub fn module(&self) -> &Module {
        self.screens.module()
    }
}

#[cfg(test)]
mod tests;
