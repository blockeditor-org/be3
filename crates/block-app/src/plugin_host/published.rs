use std::{collections::HashMap, sync::Arc};

use block_plugin_api::{
    ContactsWatch, EditorInstanceId, Watched as _, extension_of,
    ops::{Called, MethodId, Watching},
};

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

pub(super) fn offered(extension: &str) -> bool {
    extension == "be3:desktop" && cfg!(target_os = "linux")
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

    pub(super) fn take_calls<C>(&mut self, accept: &impl Fn(&Called) -> Option<C>) -> Vec<C> {
        let mut taken = Vec::new();
        self.calls.retain(|called| match accept(called) {
            Some(call) => {
                taken.push(call);
                false
            }
            None => true,
        });
        taken
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
