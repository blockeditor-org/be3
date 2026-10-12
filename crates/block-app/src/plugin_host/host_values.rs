use std::{collections::HashMap, sync::Arc};

use std::sync::RwLock;

use block_plugin_api::{EditorInstanceId, EditorMessage, HostAction, HostValue, LinkBase, Message};

pub(super) const MAX_ACTIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Anyone,
    Shell,
}

static VALUES: RwLock<Vec<(&str, Access)>> = RwLock::new(Vec::new());
static ACTIONS: RwLock<Vec<(&str, Access)>> = RwLock::new(Vec::new());

pub fn allow_value<T: HostValue>(access: Access) {
    allow(&VALUES, T::KEY, access);
}

pub fn allow_action<A: HostAction>(access: Access) {
    allow(&ACTIONS, A::KEY, access);
}

fn allow(table: &RwLock<Vec<(&'static str, Access)>>, key: &'static str, access: Access) {
    let mut table = table
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.retain(|(listed, _)| *listed != key);
    table.push((key, access));
}

fn access(table: &RwLock<Vec<(&'static str, Access)>>, key: &str) -> Option<Access> {
    if std::ptr::eq(table, &VALUES) && key == LinkBase::KEY {
        return Some(Access::Anyone);
    }
    table
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|(listed, _)| *listed == key)
        .map(|(_, access)| *access)
}

fn allowed(table: &RwLock<Vec<(&'static str, Access)>>, key: &str, shell: bool) -> bool {
    access(table, key).is_some_and(|access| shell || access == Access::Anyone)
}

fn known(table: &RwLock<Vec<(&'static str, Access)>>, key: &str) -> bool {
    access(table, key).is_some()
}

pub(super) type Published = HashMap<String, Arc<Vec<u8>>>;

#[derive(Default)]
pub(super) struct Watched {
    reported: HashMap<String, Option<Arc<Vec<u8>>>>,
    actions: Vec<(String, Vec<u8>)>,
}

impl Watched {
    pub(super) fn watch(&mut self, key: String) -> bool {
        if !known(&VALUES, &key) {
            return false;
        }
        self.reported.entry(key).or_default();
        true
    }

    pub(super) fn act(&mut self, key: String, action: Vec<u8>, shell: bool) -> bool {
        let queued = self
            .actions
            .iter()
            .filter(|(listed, _)| *listed == key)
            .count();
        if !allowed(&ACTIONS, &key, shell) || queued >= MAX_ACTIONS {
            return false;
        }
        self.actions.push((key, action));
        true
    }

    pub(super) fn watches(&self, key: &str, shell: bool) -> bool {
        self.reported.contains_key(key) && allowed(&VALUES, key, shell)
    }

    pub(super) fn forget_reported(&mut self) {
        for reported in self.reported.values_mut() {
            *reported = None;
        }
    }

    pub(super) fn take_actions(&mut self, key: &str) -> Vec<Vec<u8>> {
        let mut taken = Vec::new();
        self.actions.retain_mut(|(listed, action)| {
            if listed != key {
                return true;
            }
            taken.push(std::mem::take(action));
            false
        });
        taken
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        shell: bool,
        published: &Published,
        messages: &mut Vec<Message>,
    ) {
        let mut keys: Vec<_> = self.reported.keys().cloned().collect();
        keys.sort();
        for key in keys {
            if !allowed(&VALUES, &key, shell) {
                continue;
            }
            let Some(value) = published.get(&key) else {
                continue;
            };
            let reported = self.reported.entry(key.clone()).or_default();
            if reported
                .as_ref()
                .is_some_and(|reported| Arc::ptr_eq(reported, value))
            {
                continue;
            }
            *reported = Some(Arc::clone(value));
            messages.push(Message::Editor(EditorMessage::HostValue {
                instance,
                key,
                value: value.to_vec(),
            }));
        }
    }
}
