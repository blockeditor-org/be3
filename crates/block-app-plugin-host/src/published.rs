use stable_hash::HashMap;
use std::sync::Arc;

use block_plugin_api::{
    ContactsWatch, EditorInstanceId, Watched as _,
    ops::{Called, MethodId, Watching},
};
use block_plugin_manifest::extension_of;

use super::calls::{EditorCall, Outbound};

pub(super) const MAX_CALLS: usize = 64;

pub(super) fn watching(method: MethodId) -> Option<Watching> {
    block_plugin_api::embed::WATCHES
        .iter()
        .find(|watching| {
            (watching.open == method || watching.next == method)
                && block_plugin_api::is_published(watching.interface)
        })
        .copied()
}

pub(super) fn interface_of(called: &Called) -> Option<&str> {
    let (interface, _) = called.name.split_once('#')?;
    block_plugin_api::is_published(interface).then_some(interface)
}

const WINDOWS_DESKTOP: [&str; 3] = ["windows", "programs", "power"];

pub(super) fn offered(interface: &str) -> bool {
    if extension_of(interface) != Some("be3:desktop") {
        return false;
    }
    cfg!(target_os = "linux") || (cfg!(windows) && windows_offers(interface))
}

pub(super) fn windows_offers(interface: &str) -> bool {
    let name = interface
        .split_once('/')
        .map_or(interface, |(_, name)| name)
        .split('@')
        .next()
        .unwrap_or_default();
    WINDOWS_DESKTOP.contains(&name)
}

fn shell_only(open: MethodId) -> bool {
    open == ContactsWatch::OPEN
}

pub(super) type Published = HashMap<MethodId, Arc<Vec<u8>>>;

#[derive(Default)]
pub(super) struct Watched {
    reported: HashMap<MethodId, Option<Arc<Vec<u8>>>>,
    calls: Vec<Called>,
}

impl Watched {
    pub(super) fn watch(&mut self, open: MethodId) {
        self.reported.entry(open).or_default();
    }

    pub(super) fn call(&mut self, called: Called) -> bool {
        let queued = self
            .calls
            .iter()
            .filter(|queued| queued.name == called.name)
            .count();
        if queued >= MAX_CALLS || extension_of(&called.name).is_none() {
            return false;
        }
        self.calls.push(called);
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

    pub(super) fn take_calls(&mut self, accept: &mut dyn FnMut(&Called) -> bool) {
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
