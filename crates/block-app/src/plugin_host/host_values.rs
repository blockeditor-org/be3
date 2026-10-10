use std::{collections::HashMap, sync::Arc};

use block_plugin_api::{
    Displays, EditorInstanceId, EditorMessage, HostAction, HostValue, HostWindows, InputDevices,
    Media, MediaRequest, Message, NotificationReport, Notifications, Power, PowerAction,
    ProgramAction, Programs, ScreenLocked, WindowAction,
};

pub(super) const MAX_ACTIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    Anyone,
    Shell,
}

const VALUES: &[(&str, Access)] = &[
    (HostWindows::KEY, Access::Shell),
    (InputDevices::KEY, Access::Anyone),
    (Displays::KEY, Access::Anyone),
    (Power::KEY, Access::Anyone),
    (Media::KEY, Access::Anyone),
    (Notifications::KEY, Access::Shell),
    (ScreenLocked::KEY, Access::Anyone),
    (Programs::KEY, Access::Shell),
];

const ACTIONS: &[(&str, Access)] = &[
    (WindowAction::KEY, Access::Shell),
    (PowerAction::KEY, Access::Shell),
    (MediaRequest::KEY, Access::Shell),
    (NotificationReport::KEY, Access::Shell),
    (ProgramAction::KEY, Access::Shell),
];

fn allowed(table: &[(&str, Access)], key: &str, shell: bool) -> bool {
    table
        .iter()
        .find(|(listed, _)| *listed == key)
        .is_some_and(|(_, access)| shell || *access == Access::Anyone)
}

fn known(table: &[(&str, Access)], key: &str) -> bool {
    table.iter().any(|(listed, _)| *listed == key)
}

pub(super) type Published = HashMap<String, Arc<Vec<u8>>>;

#[derive(Default)]
pub(super) struct Watched {
    reported: HashMap<String, Option<Arc<Vec<u8>>>>,
    actions: Vec<(String, Vec<u8>)>,
}

impl Watched {
    pub(super) fn watch(&mut self, key: String) -> bool {
        if !known(VALUES, &key) {
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
        if !allowed(ACTIONS, &key, shell) || queued >= MAX_ACTIONS {
            return false;
        }
        self.actions.push((key, action));
        true
    }

    pub(super) fn watches(&self, key: &str, shell: bool) -> bool {
        self.reported.contains_key(key) && allowed(VALUES, key, shell)
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
            if !allowed(VALUES, &key, shell) {
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
