//! wasi:webgpu (decisions/be3_gpu_is_wasi_webgpu.md) and `be3:gpu/host` over the host's wgpu
//! device: the gpu every app draws with.
//!
//! Every app device here is a view of the one wgpu device: it keeps the features and limits the
//! app asked for, which creates are checked against, its own lost state and its own error
//! scopes. The host catches errors itself, in wgpu scopes of all three filters around each
//! create, queue call, `finish` and pass `end` (never a per-draw call, whose errors wgpu raises
//! at the pass's end), and files each into the innermost of the app's scopes of its kind that
//! was open when the call was made, or, with none, into the device's uncaptured errors. wgpu's
//! own scopes are per thread natively and resolve later in a browser, so a scope's error may
//! settle after the call: an app's pop waits until every call made inside it has settled.

mod limits;
mod objects;
mod passes;
mod surfaces;

use std::collections::BTreeMap;

use crate::tables::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::webgpu as convert;
use be_wasm_ops::local::{Promise, Sender, Settled};

use crate::Gpu;
use crate::tables::Table;

#[cfg(not(target_arch = "wasm32"))]
type Popped = Pin<Box<dyn Future<Output = Option<wgpu::Error>> + Send>>;
#[cfg(target_arch = "wasm32")]
type Popped = Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>;

#[cfg(not(target_arch = "wasm32"))]
type Compiling = Pin<Box<dyn Future<Output = wgpu::CompilationInfo> + Send>>;
#[cfg(target_arch = "wasm32")]
type Compiling = Pin<Box<dyn Future<Output = wgpu::CompilationInfo>>>;

/// What an object is, for its label and the device it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Device,
    Queue,
    Buffer,
    Texture,
    View,
    Sampler,
    GroupLayout,
    Group,
    PipelineLayout,
    Module,
    RenderPipeline,
    ComputePipeline,
    Encoder,
    CommandBuffer,
    RenderPass,
    ComputePass,
    QuerySet,
    BundleEncoder,
    Bundle,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Device => "a device",
            Kind::Queue => "a queue",
            Kind::Buffer => "a buffer",
            Kind::Texture => "a texture",
            Kind::View => "a texture view",
            Kind::Sampler => "a sampler",
            Kind::GroupLayout => "a bind group layout",
            Kind::Group => "a bind group",
            Kind::PipelineLayout => "a pipeline layout",
            Kind::Module => "a shader module",
            Kind::RenderPipeline => "a render pipeline",
            Kind::ComputePipeline => "a compute pipeline",
            Kind::Encoder => "a command encoder",
            Kind::CommandBuffer => "a command buffer",
            Kind::RenderPass => "a render pass",
            Kind::ComputePass => "a compute pass",
            Kind::QuerySet => "a query set",
            Kind::BundleEncoder => "a render bundle encoder",
            Kind::Bundle => "a render bundle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ErrorKind {
    Validation,
    OutOfMemory,
    Internal,
}

const KINDS: [ErrorKind; 3] = [
    ErrorKind::Validation,
    ErrorKind::OutOfMemory,
    ErrorKind::Internal,
];

impl ErrorKind {
    fn filter(self) -> wgpu::ErrorFilter {
        match self {
            ErrorKind::Validation => wgpu::ErrorFilter::Validation,
            ErrorKind::OutOfMemory => wgpu::ErrorFilter::OutOfMemory,
            ErrorKind::Internal => wgpu::ErrorFilter::Internal,
        }
    }

    fn of(filter: web::GpuErrorFilter) -> Self {
        match filter {
            web::GpuErrorFilter::Validation => ErrorKind::Validation,
            web::GpuErrorFilter::OutOfMemory => ErrorKind::OutOfMemory,
            web::GpuErrorFilter::Internal => ErrorKind::Internal,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Failure {
    kind: ErrorKind,
    message: String,
}

impl Failure {
    pub(crate) fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Validation,
            message: message.into(),
        }
    }

    fn of(error: wgpu::Error) -> Self {
        let kind = match &error {
            wgpu::Error::OutOfMemory { .. } => ErrorKind::OutOfMemory,
            wgpu::Error::Validation { .. } => ErrorKind::Validation,
            wgpu::Error::Internal { .. } => ErrorKind::Internal,
        };
        Self {
            kind,
            message: error.to_string(),
        }
    }
}

/// One of an app's error scopes.
struct Scope {
    id: u64,
    kind: ErrorKind,
    error: Option<Failure>,
    /// Calls made inside it whose errors have not settled yet.
    pending: usize,
    /// The app's pop, once it popped the scope.
    popped: Option<Promise<Result<Option<web::GpuError>, web::PopErrorScopeError>>>,
}

pub(crate) struct Device {
    features: wgpu::Features,
    limits: wgpu::Limits,
    names: Vec<web::GpuFeatureName>,
    queue: u32,
    lost: Option<(web::GpuDeviceLostReason, String)>,
    losing: Vec<Promise<web::GpuDeviceLostInfo>>,
    scopes: Vec<Scope>,
    popped: Vec<Scope>,
    uncaptured: Vec<Sender<web::GpuError>>,
}

/// The errors a call made inside a device's scopes may still raise, and where each goes.
struct Wrap {
    device: u32,
    pops: Vec<(ErrorKind, Option<u64>, Popped)>,
}

pub(crate) struct Canvas {
    surface: u32,
    configuration: Option<web::GpuCanvasConfigurationOwned>,
    size: Option<(u32, u32)>,
    /// The texture handle the frame in progress was given, which later calls in the frame name
    /// afresh.
    current: Option<wgpu::Texture>,
}

pub(crate) struct Message {
    message: String,
    kind: web::GpuCompilationMessageType,
    line: u64,
    position: u64,
    offset: u64,
    length: u64,
}

#[derive(Default)]
pub(crate) struct Web {
    gpus: Table<()>,
    adapters: Table<()>,
    infos: Table<()>,
    wgsl: Table<()>,
    features: Table<Vec<web::GpuFeatureName>>,
    limits: Table<wgpu::Limits>,
    devices: Table<Device>,
    queues: Table<u32>,
    contexts: Table<Canvas>,
    compilation_infos: Table<Vec<u32>>,
    messages: Table<Message>,
    errors: Table<Failure>,
    lost_infos: Table<(web::GpuDeviceLostReason, String)>,
    events: Table<u32>,
    constants: Table<BTreeMap<String, f64>>,
    sizes: Table<BTreeMap<String, Option<u64>>>,
    labels: HashMap<(Kind, u32), String>,
    owners: HashMap<(Kind, u32), u32>,
    buffers: HashMap<u32, web::GpuBufferUsage>,
    query_sets: HashMap<u32, (web::GpuQueryType, u32)>,
    maps: HashMap<u64, (u32, Promise<Result<(), web::MapAsyncError>>)>,
    work: HashMap<u64, Promise<()>>,
    compiling: Vec<(Compiling, Promise<web::GpuCompilationInfo>)>,
    wraps: Vec<Wrap>,
    scopes: u64,
    settled: Vec<Settled>,
    tickets: u32,
    /// Whether `tickets` has wrapped, after which a ticket may still be held.
    wrapped: bool,
    /// Uncaptured errors no listener heard, for the host's log of the app.
    logged: Vec<String>,
    /// Textures the host shares between apps (`import-texture`), which an app only samples.
    imported: HashSet<u32>,
    /// Textures the app destroyed, whose views are still made (`create-view`).
    destroyed: HashSet<u32>,
    /// The app device the call being answered is made on, whose objects alone it may use.
    acting: Option<u32>,
}

/// How many unheard errors a host keeps between reads of its log: an app that raises one a
/// frame with nobody reading would otherwise grow it without end.
const LOGGED: usize = 256;

impl Web {
    fn log(&mut self, message: String) {
        if self.logged.len() < LOGGED {
            self.logged.push(message);
        }
    }

    /// A ticket for the host's own calls to the guest (`Caller::ticket`): never `REFUSED` (0),
    /// and, once the count has wrapped, never one still waiting to settle (a device's `lost`
    /// waits for the app's whole life).
    pub(crate) fn ticket(&mut self) -> u32 {
        loop {
            let (next, wrapped) = self.tickets.overflowing_add(1);
            self.wrapped |= wrapped;
            self.tickets = next.max(1);
            if !self.wrapped || !self.waiting(self.tickets) {
                return self.tickets;
            }
        }
    }

    /// Whether a promise or sender the host holds has `ticket`.
    fn waiting(&self, ticket: u32) -> bool {
        self.maps.values().any(|(_, promise)| promise.ticket() == ticket)
            || self.work.values().any(|promise| promise.ticket() == ticket)
            || self.compiling.iter().any(|(_, promise)| promise.ticket() == ticket)
            || self.devices.values().any(|device| {
                device.losing.iter().any(|promise| promise.ticket() == ticket)
                    || device.uncaptured.iter().any(|sender| sender.ticket() == ticket)
                    || device
                        .popped
                        .iter()
                        .filter_map(|scope| scope.popped.as_ref())
                        .any(|promise| promise.ticket() == ticket)
            })
    }
}

fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>, waker: &std::task::Waker) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(waker))
}

impl Gpu {
    /// A ticket the app has not been answered with, for the generated glue.
    pub fn webgpu_ticket(&mut self) -> u32 {
        self.web.ticket()
    }

    /// Moves the ticket count to `to`, for a test of what follows when it wraps.
    #[cfg(test)]
    pub(crate) fn wind_tickets(&mut self, to: u32) {
        self.web.tickets = to;
    }

    /// The uncaptured errors since the last call that no listener of the app heard: the
    /// app's host writes them to the app's log.
    pub fn take_logged(&mut self) -> Vec<String> {
        std::mem::take(&mut self.web.logged)
    }

    pub(crate) fn owner(&self, kind: Kind, handle: u32) -> Option<u32> {
        self.web.owners.get(&(kind, handle)).copied()
    }

    pub(crate) fn own(&mut self, kind: Kind, handle: u32, device: Option<u32>, label: Option<&str>) {
        if let Some(device) = device {
            self.web.owners.insert((kind, handle), device);
        }
        if let Some(label) = label {
            self.web.labels.insert((kind, handle), label.to_owned());
        }
    }

    pub(crate) fn forget(&mut self, kind: Kind, handle: u32) {
        self.web.owners.remove(&(kind, handle));
        self.web.labels.remove(&(kind, handle));
    }

    pub(crate) fn label_of(&self, kind: Kind, handle: u32) -> String {
        self.web
            .labels
            .get(&(kind, handle))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn relabel(&mut self, kind: Kind, handle: u32, label: &str) {
        self.web.labels.insert((kind, handle), label.to_owned());
    }

    /// Whether an app device is lost or destroyed, or the host's is.
    pub(crate) fn device_lost(&self, device: u32) -> bool {
        self.settling.is_lost()
            || self
                .web
                .devices
                .get(device, "device")
                .map_or(true, |device| device.lost.is_some())
    }

    /// The features and limits an app device has.
    pub(crate) fn device_has(&self, device: u32) -> Option<(wgpu::Features, wgpu::Limits)> {
        self.web
            .devices
            .get(device, "device")
            .ok()
            .map(|device| (device.features, device.limits.clone()))
    }

    /// Raises an error on an app device: into the innermost of its scopes of the error's kind,
    /// or else to its uncaptured errors. A lost device raises nothing.
    pub(crate) fn raise(&mut self, device: Option<u32>, failure: Failure) {
        let Some(device) = device else {
            self.web.log(failure.message);
            return;
        };
        if self.device_lost(device) {
            return;
        }
        let target = self.target(device, failure.kind);
        self.file(device, target, failure);
    }

    fn target(&self, device: u32, kind: ErrorKind) -> Option<u64> {
        let device = self.web.devices.get(device, "device").ok()?;
        device
            .scopes
            .iter()
            .rev()
            .find(|scope| scope.kind == kind)
            .map(|scope| scope.id)
    }

    fn file(&mut self, device: u32, target: Option<u64>, failure: Failure) {
        let Ok(state) = self.web.devices.get_mut(device, "device") else {
            return;
        };
        if let Some(target) = target {
            let scope = state
                .scopes
                .iter_mut()
                .chain(state.popped.iter_mut())
                .find(|scope| scope.id == target);
            if let Some(scope) = scope {
                scope.error.get_or_insert(failure);
                return;
            }
        }
        if state.uncaptured.is_empty() {
            self.web.log(failure.message);
            return;
        }
        let senders: Vec<u32> = state.uncaptured.iter().map(Sender::ticket).collect();
        for ticket in senders {
            let error = web::GpuError(self.web.errors.insert(failure.clone()));
            self.web
                .settled
                .push(Sender::<web::GpuError>::new(ticket).send(&error));
        }
    }

    /// Runs `act` inside wgpu scopes of all three filters and raises what they catch on
    /// `device`, now or once it settles. An `Err` from `act` is a validation error of its own.
    pub(crate) fn wrapped<T>(
        &mut self,
        device: Option<u32>,
        act: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Option<T> {
        let guards: Vec<(ErrorKind, wgpu::ErrorScopeGuard)> = KINDS
            .iter()
            .map(|kind| (*kind, self.device.push_error_scope(kind.filter())))
            .collect();
        let outer = std::mem::replace(&mut self.web.acting, device);
        let result = act(self);
        self.web.acting = outer;
        let mut pops = Vec::new();
        for (kind, guard) in guards.into_iter().rev() {
            let target = device.and_then(|device| self.target(device, kind));
            let popped: Popped = Box::pin(guard.pop());
            pops.push((kind, target, popped));
        }
        let result = match result {
            Ok(value) => Some(value),
            Err(failure) => {
                self.raise(device, failure);
                None
            }
        };
        let waker = self.settling.waker();
        let mut waiting = Vec::new();
        for (kind, target, mut popped) in pops {
            match poll_once(popped.as_mut(), &waker) {
                Poll::Ready(error) => {
                    if let Some(error) = error {
                        match device {
                            Some(device) if !self.device_lost(device) => {
                                self.file(device, target, Failure::of(error))
                            }
                            Some(_) => {}
                            None => self.web.log(error.to_string()),
                        }
                    }
                }
                Poll::Pending => {
                    if let Some(device) = device {
                        self.waiting_in(device, target, 1);
                    }
                    waiting.push((kind, target, popped));
                }
            }
        }
        if !waiting.is_empty()
            && let Some(device) = device
        {
            self.web.wraps.push(Wrap {
                device,
                pops: waiting,
            });
        }
        result
    }

    fn waiting_in(&mut self, device: u32, target: Option<u64>, more: isize) {
        let (Some(target), Ok(state)) = (target, self.web.devices.get_mut(device, "device")) else {
            return;
        };
        if let Some(scope) = state
            .scopes
            .iter_mut()
            .chain(state.popped.iter_mut())
            .find(|scope| scope.id == target)
        {
            scope.pending = scope.pending.saturating_add_signed(more);
        }
    }

    /// Settles what the wrapped calls' scopes have caught since, and the pops waiting on them.
    fn settle_wraps(&mut self) {
        let waker = self.settling.waker();
        let wraps = std::mem::take(&mut self.web.wraps);
        for mut wrap in wraps {
            let mut waiting = Vec::new();
            for (kind, target, mut popped) in wrap.pops.drain(..) {
                match poll_once(popped.as_mut(), &waker) {
                    Poll::Ready(error) => {
                        self.waiting_in(wrap.device, target, -1);
                        if let Some(error) = error
                            && !self.device_lost(wrap.device)
                        {
                            self.file(wrap.device, target, Failure::of(error));
                        }
                    }
                    Poll::Pending => waiting.push((kind, target, popped)),
                }
            }
            if !waiting.is_empty() {
                wrap.pops = waiting;
                self.web.wraps.push(wrap);
            }
        }
        let devices: Vec<u32> = self.web.devices.handles().collect();
        for device in devices {
            self.settle_pops(device);
        }
    }

    fn settle_pops(&mut self, device: u32) {
        let Ok(state) = self.web.devices.get_mut(device, "device") else {
            return;
        };
        let (ready, waiting): (Vec<Scope>, Vec<Scope>) = std::mem::take(&mut state.popped)
            .into_iter()
            .partition(|scope| scope.pending == 0);
        state.popped = waiting;
        for scope in ready {
            let error = scope
                .error
                .map(|failure| web::GpuError(self.web.errors.insert(failure)));
            if let Some(promise) = scope.popped {
                self.web.settled.push(promise.settle(&Ok(error)));
            }
        }
    }

    pub(crate) fn push_scope(&mut self, device: u32, filter: web::GpuErrorFilter) {
        self.web.scopes += 1;
        let id = self.web.scopes;
        if let Ok(state) = self.web.devices.get_mut(device, "device") {
            state.scopes.push(Scope {
                id,
                kind: ErrorKind::of(filter),
                error: None,
                pending: 0,
                popped: None,
            });
        }
    }

    pub(crate) fn pop_scope(
        &mut self,
        device: u32,
        promise: Promise<Result<Option<web::GpuError>, web::PopErrorScopeError>>,
    ) {
        let lost = self.device_lost(device);
        let state = match self.web.devices.get_mut(device, "device") {
            Ok(state) => state,
            Err(message) => {
                let refused = web::PopErrorScopeError {
                    kind: web::PopErrorScopeErrorKind::OperationError,
                    message,
                };
                self.web.settled.push(promise.settle(&Err(refused)));
                return;
            }
        };
        match state.scopes.pop() {
            // A lost device's pops answer no error, as WebGPU's do.
            Some(_) if lost => self.web.settled.push(promise.settle(&Ok(None))),
            Some(mut scope) => {
                scope.popped = Some(promise);
                state.popped.push(scope);
                self.settle_pops(device);
            }
            None => self.web.settled.push(promise.settle(&Err(web::PopErrorScopeError {
                kind: web::PopErrorScopeErrorKind::OperationError,
                message: "there is no error scope to pop".to_owned(),
            }))),
        }
    }

    /// Settles a device's `lost` promises once it is lost, by the app or the host.
    fn settle_losses(&mut self) {
        let host = self.settling.lost();
        let devices: Vec<u32> = self.web.devices.handles().collect();
        for device in devices {
            let Ok(state) = self.web.devices.get_mut(device, "device") else {
                continue;
            };
            if state.lost.is_none()
                && let Some(message) = &host
            {
                state.lost = Some((web::GpuDeviceLostReason::Unknown, message.clone()));
            }
            let Some(lost) = state.lost.clone() else {
                continue;
            };
            let losing = std::mem::take(&mut state.losing);
            let senders = std::mem::take(&mut state.uncaptured);
            let scopes: Vec<Scope> = state.scopes.drain(..).chain(state.popped.drain(..)).collect();
            for promise in losing {
                let info = web::GpuDeviceLostInfo(self.web.lost_infos.insert(lost.clone()));
                self.web.settled.push(promise.settle(&info));
            }
            for sender in senders {
                self.web.settled.push(sender.end());
            }
            for scope in scopes {
                if let Some(promise) = scope.popped {
                    self.web.settled.push(promise.settle(&Ok(None)));
                }
            }
        }
    }

    pub(crate) fn settle_web(&mut self, wait: bool) -> Vec<Settled> {
        // Something settled already answers a waiting poll.
        let wait = wait && self.web.settled.is_empty();
        if let Err(error) = self.settling.poll(&self.device, wait) {
            self.web.log(format!("the gpu could not be polled: {error}"));
        }
        let (maps, work_done) = self.settling.take_settled();
        let mut device_maps = Vec::new();
        for (ticket, mapped) in maps {
            match self.web.maps.remove(&ticket) {
                Some((buffer, promise)) => {
                    let answer = match mapped {
                        true => Ok(()),
                        false => Err(web::MapAsyncError {
                            kind: web::MapAsyncErrorKind::AbortError,
                            message: format!("buffer {buffer} was unmapped or destroyed before it mapped, or its map failed"),
                        }),
                    };
                    self.web.settled.push(promise.settle(&answer));
                }
                None => device_maps.push((ticket, mapped)),
            }
        }
        let mut device_work = Vec::new();
        for ticket in work_done {
            match self.web.work.remove(&ticket) {
                Some(promise) => self.web.settled.push(promise.settle(&())),
                None => device_work.push(ticket),
            }
        }
        self.settling.give_back(device_maps, device_work);
        let waker = self.settling.waker();
        let compiling = std::mem::take(&mut self.web.compiling);
        for (mut compilation, promise) in compiling {
            match poll_once(compilation.as_mut(), &waker) {
                Poll::Ready(info) => {
                    let handle = self.compilation_info(&info);
                    self.web.settled.push(promise.settle(&handle));
                }
                Poll::Pending => self.web.compiling.push((compilation, promise)),
            }
        }
        self.settle_wraps();
        self.settle_losses();
        std::mem::take(&mut self.web.settled)
    }

    fn compilation_info(&mut self, info: &wgpu::CompilationInfo) -> web::GpuCompilationInfo {
        let mut messages = Vec::new();
        for message in &info.messages {
            let (line, position, offset, length) = match message.location {
                Some(location) => (
                    u64::from(location.line_number),
                    u64::from(location.line_position),
                    u64::from(location.offset),
                    u64::from(location.length),
                ),
                None => (0, 0, 0, 0),
            };
            messages.push(self.web.messages.insert(Message {
                message: message.message.clone(),
                kind: Convert::convert(message.message_type),
                line,
                position,
                offset,
                length,
            }));
        }
        web::GpuCompilationInfo(self.web.compilation_infos.insert(messages))
    }

    pub(crate) fn new_device(
        &mut self,
        descriptor: Option<web::GpuDeviceDescriptor>,
    ) -> Result<web::GpuDevice, web::RequestDeviceError> {
        let refuse = |kind, message: String| web::RequestDeviceError { kind, message };
        let offered = self.device.features();
        let mut features = wgpu::Features::empty();
        let mut names = vec![web::GpuFeatureName::CoreFeaturesAndLimits];
        let descriptor = descriptor.unwrap_or(web::GpuDeviceDescriptor {
            required_features: None,
            required_limits: None,
            default_queue: None,
            label: None,
        });
        for name in descriptor.required_features.unwrap_or_default() {
            match convert::feature(name) {
                Some(feature) if offered.contains(feature) => {
                    features |= feature;
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                _ => {
                    return Err(refuse(
                        web::RequestDeviceErrorKind::TypeError,
                        format!(
                            "the adapter does not have the feature {}",
                            convert::feature_name(name)
                        ),
                    ));
                }
            }
        }
        let host = self.device.limits();
        let mut limits = floor(&host);
        if let Some(record) = descriptor.required_limits {
            let asked = self.web.sizes.get(record.0, "record").cloned().map_err(|message| {
                refuse(web::RequestDeviceErrorKind::TypeError, message)
            })?;
            for (name, value) in asked {
                let Some(value) = value else {
                    continue;
                };
                limit(&mut limits, &host, &name, value)
                    .map_err(|message| refuse(web::RequestDeviceErrorKind::OperationError, message))?;
            }
        }
        let queue = self.web.queues.insert(0);
        let device = self.web.devices.insert(Device {
            features,
            limits,
            names,
            queue,
            lost: None,
            losing: Vec::new(),
            scopes: Vec::new(),
            popped: Vec::new(),
            uncaptured: Vec::new(),
        });
        if let Ok(owner) = self.web.queues.get_mut(queue, "queue") {
            *owner = device;
        }
        self.own(Kind::Device, device, Some(device), descriptor.label.as_deref());
        let queue_label = descriptor
            .default_queue
            .and_then(|queue| queue.label);
        self.own(Kind::Queue, queue, Some(device), queue_label.as_deref());
        Ok(web::GpuDevice(device))
    }

    /// Destroys an app device: frees the buffers and textures made on it and fails their
    /// pending maps; the host's device and other app devices go on.
    pub(crate) fn destroy_web_device(&mut self, device: u32) {
        let Ok(state) = self.web.devices.get_mut(device, "device") else {
            return;
        };
        if state.lost.is_some() {
            return;
        }
        state.lost = Some((
            web::GpuDeviceLostReason::Destroyed,
            "the app destroyed its device".to_owned(),
        ));
        let owned: Vec<(Kind, u32)> = self
            .web
            .owners
            .iter()
            .filter(|(_, owner)| **owner == device)
            .map(|(object, _)| *object)
            .collect();
        for (kind, handle) in owned {
            match kind {
                Kind::Buffer => {
                    self.settling.forget(handle);
                    if let Ok(buffer) = self.buffers.get(handle, "buffer") {
                        buffer.destroy();
                    }
                }
                Kind::Texture if self.made_textures.contains(&handle) => {
                    if let Ok(texture) = self.textures.get(handle, "texture") {
                        texture.destroy();
                        self.web.destroyed.insert(handle);
                    }
                }
                _ => {}
            }
        }
        self.settling.wake();
    }
}

/// The limits a device is made with when it asks for none: WebGPU's defaults, or the adapter's
/// where it has worse.
fn floor(host: &wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::default();
    macro_rules! floored {
        ($($name:literal => $field:ident),* $(,)?) => {
            $(
                limits.$field = match $name.starts_with("min") {
                    true => limits.$field.max(host.$field),
                    false => limits.$field.min(host.$field),
                };
            )*
        };
    }
    be_wasm_gpu_convert::each_limit!(floored);
    limits
}

/// Raises one limit of `limits` to `value`, as `requiredLimits` asks: no further than the
/// adapter's, and a value worse than the default keeps the default. An alignment is better
/// the smaller it is, and is a power of two.
fn limit(limits: &mut wgpu::Limits, host: &wgpu::Limits, name: &str, value: u64) -> Result<(), String> {
    macro_rules! raised {
        ($($name:literal => $field:ident),* $(,)?) => {
            match name {
                $($name => {
                    let offered = u64::from(host.$field);
                    let alignment = $name.starts_with("min");
                    if alignment && !value.is_power_of_two() {
                        return Err(format!("{} must be a power of two, not {value}", $name));
                    }
                    let allowed = match alignment {
                        true => value >= offered,
                        false => value <= offered,
                    };
                    if !allowed {
                        return Err(format!("the adapter's {} is {offered}, not {value}", $name));
                    }
                    let value = value
                        .try_into()
                        .map_err(|_| format!("{value} is too large for {}", $name))?;
                    limits.$field = match alignment {
                        true => limits.$field.min(value),
                        false => limits.$field.max(value),
                    };
                    Ok(())
                })*
                // A limit wgpu has no field for: WebGPU's per-stage storage limits, which a
                // core device has at the per-shader-stage limit.
                "maxStorageBuffersInVertexStage"
                | "maxStorageBuffersInFragmentStage"
                | "maxStorageTexturesInVertexStage"
                | "maxStorageTexturesInFragmentStage" => Ok(()),
                other => Err(format!("{other} is not a limit")),
            }
        };
    }
    be_wasm_gpu_convert::each_limit!(raised)
}

use be_wasm_gpu_convert::Convert;
