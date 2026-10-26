use std::error::Error;
use std::ffi::CStr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use beui_core::app::Waker;
use beui_renderer_wgpu::present::{OpenDevice, OpenGpu, plain_instance};

use crate::Status;
use crate::headset::{Missing, RETRY, Runtime, Xr};

#[derive(Clone)]
pub struct Link {
    inner: Arc<Inner>,
    _users: Option<Arc<Users>>,
}

struct Users(Arc<Inner>);

impl Drop for Users {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.stopped = true;
        state.found = None;
        state.opened = None;
        drop(state);
        self.0.changed.notify_all();
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct Inner {
    application: String,
    state: Mutex<State>,
    changed: Condvar,
}

struct State {
    found: Option<Xr>,
    opened: Option<Xr>,
    status: Status,
    waker: Option<Waker>,
    stopped: bool,
}

impl Link {
    pub fn new(application: &str) -> Self {
        let inner = Arc::new(Inner {
            application: application.to_owned(),
            state: Mutex::new(State {
                found: None,
                opened: None,
                status: Status::Missing(String::new()),
                waker: None,
                stopped: false,
            }),
            changed: Condvar::new(),
        });
        let link = Self {
            inner: inner.clone(),
            _users: Some(Arc::new(Users(inner.clone()))),
        };
        let kept = link.attempt(None);
        match link.status() {
            Status::Found(runtime) => eprintln!("beui-layer-xr: showing space on {runtime}"),
            Status::Missing(why) | Status::Failed(why) => {
                eprintln!("beui-layer-xr: waiting for a headset: {why}")
            }
        }
        let searching = Self {
            inner,
            _users: None,
        };
        let spawned = std::thread::Builder::new()
            .name("beui-layer-xr search".to_owned())
            .spawn(move || searching.search(kept));
        if let Err(error) = spawned {
            link.fail(format!("the headset search did not start: {error}"));
        }
        link
    }

    pub fn status(&self) -> Status {
        self.state().status.clone()
    }

    pub fn opened(&self) -> Option<Xr> {
        self.state().opened.clone()
    }

    pub fn open_device(&self, extensions: &'static [&'static CStr]) -> OpenDevice {
        OpenDevice {
            extensions,
            gpu: Some(Arc::new(self.clone())),
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        self.state().waker = Some(waker);
    }

    pub fn lost(&self, xr: &Xr) {
        let mut state = self.state();
        if state.opened.as_ref().is_some_and(|opened| opened.same(xr)) {
            state.opened = None;
        }
        if state.found.as_ref().is_some_and(|found| found.same(xr)) {
            state.found = None;
            state.status = Status::Missing(format!("{} stopped", xr.runtime()));
        }
        drop(state);
        self.inner.changed.notify_all();
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner.lock()
    }

    fn search(&self, mut kept: Option<Runtime>) {
        loop {
            let mut state = self.state();
            while !state.stopped
                && (state.found.is_some() || matches!(state.status, Status::Failed(_)))
            {
                state = self
                    .inner
                    .changed
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            if state.stopped {
                return;
            }
            drop(state);
            kept = self.attempt(kept.take());
            let state = self.state();
            match state.found.as_ref().map(|xr| xr.runtime().to_owned()) {
                Some(runtime) => eprintln!("beui-layer-xr: found a headset on {runtime}"),
                None => drop(self.inner.changed.wait_timeout(state, RETRY)),
            }
        }
    }

    fn attempt(&self, kept: Option<Runtime>) -> Option<Runtime> {
        let runtime = match kept {
            Some(runtime) => Ok(runtime),
            None => Runtime::open(&self.inner.application).map_err(Missing::Runtime),
        };
        let (status, kept) = match runtime.and_then(Runtime::system) {
            Ok(xr) => {
                let status = Status::Found(xr.runtime().to_owned());
                let mut state = self.state();
                if state.stopped {
                    return None;
                }
                state.found = Some(xr);
                (status, None)
            }
            Err(Missing::Headset(runtime, why)) => (Status::Missing(why), Some(runtime)),
            Err(Missing::Runtime(why)) => (Status::Missing(why), None),
        };
        self.change(status);
        kept
    }

    fn fail(&self, why: String) {
        eprintln!("beui-layer-xr: {why}");
        let mut state = self.state();
        state.found = None;
        state.opened = None;
        drop(state);
        self.change(Status::Failed(why));
    }

    fn change(&self, status: Status) {
        let mut state = self.state();
        if state.status == status {
            return;
        }
        state.status = status;
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl OpenGpu for Link {
    fn adapter(&self) -> Result<(wgpu::Instance, Option<wgpu::Adapter>), Box<dyn Error>> {
        let found = {
            let mut state = self.state();
            state.opened = None;
            state.found.clone()
        };
        if let Some(xr) = found {
            match xr.adapter() {
                Ok((instance, adapter)) => {
                    self.state().opened = Some(xr);
                    return Ok((instance, Some(adapter)));
                }
                Err(error) => self.fail(format!("{} picked no usable GPU: {error}", xr.runtime())),
            }
        }
        Ok((plain_instance(), None))
    }

    fn device(
        &self,
        adapter: &wgpu::Adapter,
        extensions: &[&'static CStr],
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<Option<(wgpu::Device, wgpu::Queue)>, Box<dyn Error>> {
        let Some(xr) = self.opened() else {
            return Ok(None);
        };
        match xr.device(adapter, extensions, descriptor) {
            Ok(opened) => Ok(Some(opened)),
            Err(error) => {
                self.fail(format!("{} could not share the GPU: {error}", xr.runtime()));
                Ok(None)
            }
        }
    }

    fn stale(&self) -> bool {
        let state = self.state();
        match (&state.found, &state.opened) {
            (Some(found), Some(opened)) => !found.same(opened),
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}
