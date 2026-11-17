use std::time::Duration;

use be_sdk_api::{HelloAccepted, ScreenDamage, ops::Outcome};
use be_sdk_manifest::AppManifest;

use super::calls::{Heard, Outbound};
use super::wire::Wire;
use be_host_base::host;
use be_host_stack::discovery;
use be_wasm_guest_host::Worker;
pub(super) use be_wasm_guest_host::{Deadline, StepTime};

#[cfg(not(target_arch = "wasm32"))]
use super::wasm as platform;
#[cfg(target_arch = "wasm32")]
use super::web as platform;

pub(super) const NOT_INSTALLED: &str = "The app host is not installed.";
const NO_ENTRY_POINT: &str = "This app has no wasm entry point.";

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

/// Where the worker `module` of the staged app with the id `app_id` is staged, relative to the
/// staged apps, if its manifest names it under `workers`: the one place that maps a worker's name
/// to a file on every platform, so any other name, a path or another app's module, is refused.
pub(super) fn worker_file(app_id: &str, module: &str) -> Result<String, String> {
    let manifest = discovery::manifests()
        .into_iter()
        .find(|manifest| manifest.identity.id == app_id)
        .ok_or_else(|| format!("no staged app has the id {app_id}"))?;
    if manifest.workers.iter().any(|worker| worker == module) {
        return Ok(worker_path(app_id, module));
    }
    Err(format!("{app_id} stages no worker named {module:?}"))
}

/// Where a worker module of the app `app_id` is staged (crates/be-tool-buck's stage).
pub(super) fn worker_path(app_id: &str, worker: &str) -> String {
    format!("workers/{app_id}/{worker}")
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
    pub(super) fn new(app: &AppManifest) -> Self {
        Self {
            module: discovery::entry_point(&app.identity.id, &app.entry_point),
            worker: None,
            mailbox: None,
            wire: platform::wire(app),
            started: host::now(),
            error: None,
        }
    }

    pub(super) fn start(&mut self, app: &AppManifest) {
        self.shutdown();
        self.started = host::now();
        self.error = None;
        self.wire = platform::wire(app);
        let Some(module) = self.module.clone() else {
            self.error = Some(NO_ENTRY_POINT.to_owned());
            return;
        };
        match platform::spawn(app, module) {
            Ok((channel, mailbox, waiting)) => {
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

    pub(super) fn send(&mut self, outbound: Vec<Outbound>) {
        self.wired(|wire| {
            for outbound in outbound {
                match outbound {
                    Outbound::Open(opening) => wire.open(*opening),
                    Outbound::Close(instance) => wire.close(instance),
                    Outbound::Event(instance, event) => wire.event(instance, event),
                    Outbound::Editor(instance, call) => wire.call(instance, call),
                    Outbound::App(call) => wire.app_call(call),
                }
            }
            wire.flush_hearing();
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
        self.wire.poll_sandbox();
        if let Some(failure) = worker.queue(self.wire.take_outbound()) {
            self.fail(failure);
        }
        messages
    }

    #[cfg(test)]
    pub(super) fn wire_mut(&mut self) -> &mut Wire {
        &mut self.wire
    }

    pub(super) fn waiting(&self) -> Vec<be_sdk_api::ops::OpId> {
        self.wire.waiting()
    }

    pub(super) fn answer_watches(
        &mut self,
        answers: Vec<(be_sdk_api::ops::OpId, Vec<u8>)>,
        accepted: Vec<be_sdk_api::ops::OpId>,
    ) {
        for (watch, value) in answers {
            self.wire.answer_watch(watch, value);
        }
        for send in accepted {
            self.wire.accepted(send);
        }
        self.flush();
    }

    pub(super) fn relay(&mut self, replica: be_sdk_api::ops::OpId, frame: &Vec<u8>) -> bool {
        self.wire.relay(replica, frame)
    }

    pub(super) fn stream_item(&mut self, stream: be_sdk_api::ops::OpId, item: &[u8]) -> bool {
        self.wire.stream_item(stream, item)
    }

    pub(super) fn answer(&mut self, answers: Vec<(be_sdk_api::ops::OpId, Outcome)>) {
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
        let worker = self.worker.as_mut()?;
        // The presenter lets an app's textures go by its layout (`SurfacePresenter::retain`), so
        // the surfaces it released need nothing more here.
        worker.take_released();
        let (presented, presents) = worker.take_presented()?;
        Some(platform::frame(presented, presents))
    }

    pub(super) fn holds_input(&self) -> bool {
        self.wire.holds_input()
    }

    pub(super) fn settled(&self) -> bool {
        self.worker.as_ref().is_none_or(Worker::settled)
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
        self.wire.stop_sandbox();
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
    pub(super) fn place(
        &mut self,
        placements: Vec<super::web::Placement>,
        layers: Option<super::web::Overlay>,
    ) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        if let Err(error) = worker.channel.place(placements, layers) {
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

    /// Stops the app with `failure`, and every worker it started with it.
    fn fail(&mut self, failure: String) {
        self.error.get_or_insert(failure);
        self.shutdown();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

pub(super) fn prepare(app: &AppManifest) {
    if let Some(module) = discovery::entry_point(&app.identity.id, &app.entry_point) {
        platform::prepare(app, module);
    }
}

pub(super) fn hand_over_device() {
    platform::hand_over_device();
}

pub(super) fn discard_prepared() {
    platform::discard_prepared();
}

pub(super) struct Availability(pub(super) Result<(), String>);

impl Availability {
    pub(super) fn missing() -> Self {
        Self(Err(NOT_INSTALLED.to_owned()))
    }
}

/// The sandbox an app is served on its wire for its workers (`Server::for_app`).
pub(super) struct AppSandbox(pub(super) be_wasm_sandbox_host::Server);

impl be_sdk_app_host::sandbox::Sandbox for AppSandbox {
    fn call(
        &mut self,
        call: be_wasm_sandbox_api::embed::sandbox::Call,
        connection: &mut be_sdk_api::ops::Connection,
    ) {
        self.0.call(call, connection);
    }

    fn canceled(
        &mut self,
        op: be_sdk_api::ops::OpId,
        connection: &mut be_sdk_api::ops::Connection,
    ) -> bool {
        self.0.canceled(op, connection)
    }

    fn poll(&mut self, connection: &mut be_sdk_api::ops::Connection) {
        self.0.poll(connection);
    }

    fn stop(self: Box<Self>, why: &str, connection: &mut be_sdk_api::ops::Connection) {
        self.0.stop(why, connection);
    }
}
