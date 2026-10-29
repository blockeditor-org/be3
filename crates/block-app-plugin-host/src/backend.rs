use std::{sync::Arc, time::Duration};

use block_plugin_api::{HelloAccepted, ScreenDamage, ops::Outcome};
use block_plugin_manifest::PluginManifest;

use super::calls::{Heard, Outbound};
use super::wire::Wire;
use block_app_base::host;
use block_app_stack::discovery;
use block_guest_host::Worker;
pub(super) use block_guest_host::{Deadline, StepTime};

#[cfg(not(target_arch = "wasm32"))]
use super::wasm as platform;
#[cfg(target_arch = "wasm32")]
use super::web as platform;

pub(super) const NOT_INSTALLED: &str = "The plugin host is not installed.";
const NO_ENTRY_POINT: &str = "This plugin has no wasm entry point.";

pub(super) type Frame = platform::Frame;

pub(super) type Picture = platform::Picture;

pub(super) fn release(picture: &Picture) {
    platform::release(picture);
}

pub(super) trait ShownFrame {
    fn presents(&self) -> u64;

    fn damage(&self) -> Option<&[ScreenDamage]>;

    fn set_damage(&mut self, damage: Option<Vec<ScreenDamage>>);

    fn composited(&self) -> bool {
        true
    }
}

pub(super) struct Backend {
    module: Option<String>,
    worker: Option<Worker<platform::Channel>>,
    mailbox: Option<platform::Mailbox>,
    wire: Wire,
    started: Duration,
    error: Option<String>,
}

impl Backend {
    pub(super) fn new(plugin: &PluginManifest) -> Self {
        Self {
            module: discovery::entry_point(&plugin.identity.id, &plugin.entry_point),
            worker: None,
            mailbox: None,
            wire: Wire::granting(plugin.extensions.clone()),
            started: host::now(),
            error: None,
        }
    }

    pub(super) fn start(&mut self, plugin: &PluginManifest) {
        self.shutdown();
        self.started = host::now();
        self.error = None;
        self.wire = Wire::granting(plugin.extensions.clone());
        let Some(module) = self.module.clone() else {
            self.error = Some(NO_ENTRY_POINT.to_owned());
            return;
        };
        let waiting = Arc::default();
        match platform::spawn(plugin, module, Arc::clone(&waiting)) {
            Ok((channel, mailbox)) => {
                self.worker = Some(Worker::new(channel, waiting));
                self.mailbox = Some(mailbox);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn ready(&self) -> bool {
        self.worker.as_ref().is_some_and(Worker::ready)
    }

    fn wired(&mut self, act: impl FnOnce(&mut Wire)) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        act(&mut self.wire);
        if let Some(failure) = worker.queue(self.wire.take_outbound()) {
            self.fail(failure);
        }
    }

    pub(super) fn send(&mut self, outbound: Outbound) {
        self.wired(|wire| match outbound {
            Outbound::Open(opening) => wire.open(*opening),
            Outbound::Close(instance) => wire.close(instance),
            Outbound::Editor(instance, call) => wire.call(instance, call),
            Outbound::Plugin(call) => wire.plugin_call(call),
        });
    }

    pub(super) fn accept(&mut self, accepted: &HelloAccepted) {
        self.wired(|wire| wire.accept(accepted));
    }

    pub(super) fn ask_to_shut_down(&mut self) {
        self.wired(Wire::shutdown);
    }

    pub(super) fn receive(&mut self) -> Vec<Heard> {
        self.poll();
        let Some(worker) = &mut self.worker else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        for frame in worker.take_received() {
            if let Err(failure) = self.wire.receive(&frame, &mut messages) {
                self.error.get_or_insert(failure);
            }
        }
        if let Some(failure) = worker.queue(self.wire.take_outbound()) {
            self.fail(failure);
        }
        messages
    }

    #[cfg(test)]
    pub(super) fn wire_mut(&mut self) -> &mut Wire {
        &mut self.wire
    }

    pub(super) fn waiting(&self) -> Vec<block_plugin_api::ops::OpId> {
        self.wire.waiting()
    }

    pub(super) fn answer_watches(
        &mut self,
        answers: Vec<(block_plugin_api::ops::OpId, Vec<u8>)>,
        accepted: Vec<block_plugin_api::ops::OpId>,
    ) {
        for (watch, value) in answers {
            self.wire.answer_watch(watch, value);
        }
        for send in accepted {
            self.wire.accepted(send);
        }
        self.flush();
    }

    pub(super) fn relay(&mut self, replica: block_plugin_api::ops::OpId, frame: &Vec<u8>) -> bool {
        self.wire.relay(replica, frame)
    }

    pub(super) fn answer(&mut self, answers: Vec<(block_plugin_api::ops::OpId, Outcome)>) {
        for (op, outcome) in answers {
            self.wire.answer(op, outcome);
        }
        self.flush();
    }

    fn flush(&mut self) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        if !self.wire.has_outbound() {
            return;
        }
        if let Some(failure) = worker.queue(self.wire.take_outbound()) {
            self.fail(failure);
        }
    }

    pub(super) fn received_frame(&mut self) -> Option<Frame> {
        let (presented, presents) = self.worker.as_mut()?.take_presented()?;
        Some(platform::frame(presented, presents))
    }

    pub(super) fn settled(&self) -> bool {
        self.worker.as_ref().is_none_or(Worker::settled)
    }

    pub(super) fn wait(&mut self, deadline: Deadline) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        match worker.wait(deadline) {
            Ok(woken) => woken,
            Err(failure) => {
                self.fail(failure);
                true
            }
        }
    }

    pub(super) fn took(&mut self) -> Option<StepTime> {
        self.worker.as_mut()?.take_took()
    }

    pub(super) fn stepped(&mut self) -> (u64, Duration) {
        self.worker
            .as_mut()
            .map_or((0, Duration::ZERO), |worker| worker.take_stepped())
    }

    pub(super) fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    pub(super) fn state(&self) -> &'static str {
        match &self.worker {
            Some(worker) if worker.ready() => "running",
            Some(_) => "starting",
            None => "stopped",
        }
    }

    pub(super) fn uptime(&self) -> Option<Duration> {
        self.worker
            .is_some()
            .then(|| host::now().saturating_sub(self.started))
    }

    pub(super) fn shutdown(&mut self) {
        self.worker = None;
        self.mailbox = None;
    }

    pub(super) fn offer(&mut self, image: u64, picture: &Picture) {
        if let (Some(worker), Some(mailbox)) = (&mut self.worker, &self.mailbox) {
            platform::offer(mailbox, &mut worker.channel, image, picture);
        }
    }

    pub(super) fn withdraw(&mut self, image: u64) {
        if let (Some(worker), Some(mailbox)) = (&mut self.worker, &self.mailbox) {
            platform::withdraw(mailbox, &mut worker.channel, image);
        }
    }

    pub(super) fn export(&mut self, surfaces: Vec<u32>) {
        if let Some(worker) = &mut self.worker {
            platform::export(&mut worker.channel, surfaces);
        }
    }

    pub(super) fn pictures(frame: &Frame, surfaces: &[u32]) -> Vec<(u32, Picture)> {
        platform::pictures(frame, surfaces)
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn place(&mut self, placements: Vec<super::web::Placement>) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        if let Err(error) = worker.channel.place(placements) {
            self.error.get_or_insert(error);
        }
    }

    fn poll(&mut self) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        if let Some(failure) = worker.poll() {
            self.fail(failure);
            return;
        }
        match worker.hung(host::now()) {
            Ok(Some(left)) => host::request_repaint_after(left),
            Ok(None) => {}
            Err(failure) => self.fail(failure),
        }
    }

    fn fail(&mut self, failure: String) {
        self.error.get_or_insert(failure);
        self.worker = None;
        self.mailbox = None;
    }
}

pub(super) struct Availability(pub(super) Result<(), String>);

impl Availability {
    pub(super) fn missing() -> Self {
        Self(Err(NOT_INSTALLED.to_owned()))
    }
}
