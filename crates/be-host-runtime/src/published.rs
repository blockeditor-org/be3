use be_lib_stable_hash::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use be_sdk_api::{
    ContactsWatch, EditorInstanceId, Watched as _, WindowsWatch,
    ops::{MethodId, Started, Watching},
};
use be_sdk_manifest::extension_of;

use super::calls::{EditorCall, Outbound};

pub(super) const MAX_CALLS: usize = 64;

pub(super) fn watching(method: MethodId) -> Option<Watching> {
    be_sdk_api::embed::WATCHES
        .iter()
        .find(|watching| {
            (watching.open == method || watching.next == method)
                && be_sdk_api::is_published(watching.interface)
        })
        .copied()
}

pub(super) fn interface_of(started: &Started) -> Option<&'static str> {
    let (interface, _) = be_sdk_api::embed::method_name(started.method)?.split_once('#')?;
    be_sdk_api::is_published(interface).then_some(interface)
}

const WINDOWS_DESKTOP: [&str; 3] = ["windows", "programs", "power"];

const PROGRAM_WINDOWS: [&str; 2] = ["windows", "programs"];

static DESKTOP: AtomicBool = AtomicBool::new(false);

pub fn run_as_desktop() {
    DESKTOP.store(true, Ordering::Relaxed);
}

pub(super) fn offered(interface: &str) -> bool {
    if extension_of(interface) != Some("be3:desktop") {
        return false;
    }
    offers(
        interface,
        Platform::current(),
        DESKTOP.load(Ordering::Relaxed),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Platform {
    Linux,
    Windows,
    Other,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

pub(super) fn offers(interface: &str, platform: Platform, desktop: bool) -> bool {
    let name = interface
        .split_once('/')
        .map_or(interface, |(_, name)| name)
        .split('@')
        .next()
        .unwrap_or_default();
    match platform {
        Platform::Linux => desktop || !PROGRAM_WINDOWS.contains(&name),
        Platform::Windows => desktop && WINDOWS_DESKTOP.contains(&name),
        Platform::Other => false,
    }
}

fn shell_only(open: MethodId) -> bool {
    open == ContactsWatch::OPEN || open == WindowsWatch::OPEN
}

fn shell_only_call(started: &Started) -> bool {
    interface_of(started) == Some(WindowsWatch::INTERFACE)
}

pub(super) type Published = HashMap<MethodId, Arc<Vec<u8>>>;

#[derive(Default)]
pub(super) struct Watched {
    reported: HashMap<MethodId, Option<Arc<Vec<u8>>>>,
    calls: Vec<Started>,
}

impl Watched {
    /// A watch that `starts_anew` is not told what was published before it opened.
    pub(super) fn watch(&mut self, open: MethodId, published: Option<&Arc<Vec<u8>>>) {
        let reported = self.reported.entry(open).or_default();
        if be_sdk_api::starts_anew(open) {
            *reported = published.cloned();
        }
    }

    pub(super) fn unwatch(&mut self, open: MethodId) {
        self.reported.remove(&open);
    }

    pub(super) fn call(&mut self, started: Started, shell: bool) -> bool {
        if !shell && shell_only_call(&started) {
            return false;
        }
        let queued = self
            .calls
            .iter()
            .filter(|queued| queued.method == started.method)
            .count();
        if queued >= MAX_CALLS || interface_of(&started).and_then(extension_of).is_none() {
            return false;
        }
        self.calls.push(started);
        true
    }

    pub(super) fn watches(&self, open: MethodId, shell: bool) -> bool {
        self.reported.contains_key(&open) && (shell || !shell_only(open))
    }

    pub(super) fn forget_reported(&mut self) {
        for reported in self.reported.values_mut() {
            *reported = None;
        }
    }

    pub(super) fn take_calls(&mut self, accept: &mut dyn FnMut(&Started) -> bool) {
        self.calls.retain(|called| !accept(called));
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        shell: bool,
        published: &Published,
        messages: &mut Vec<Outbound>,
    ) {
        let mut opens: Vec<_> = self.reported.keys().copied().collect();
        opens.sort();
        for open in opens {
            if !shell && shell_only(open) {
                continue;
            }
            let Some(value) = published.get(&open) else {
                continue;
            };
            let reported = self.reported.entry(open).or_default();
            if reported
                .as_ref()
                .is_some_and(|reported| Arc::ptr_eq(reported, value))
            {
                continue;
            }
            *reported = Some(Arc::clone(value));
            messages.push(Outbound::Editor(
                instance,
                EditorCall::Watched {
                    open,
                    value: Arc::clone(value),
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests;
