use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use beui::reactive::*;
use beui::{Forwarded, ForwardedInput, PointerPhase, Rect};
use block_plugin_api::{EditorInstanceId, EditorRegion};

use crate::plugin_host::HostChildStatus;

#[derive(Clone, Copy)]
pub(crate) struct DesktopPlace {
    pub(crate) display: Memo<Rect>,
    pub(crate) desktop: Memo<Rect>,
    pub(crate) displays: Memo<Vec<Rect>>,
    pub(crate) scale: Memo<f32>,
    pub(crate) primary: Memo<bool>,
}

#[derive(Clone, Copy)]
pub(crate) struct DesktopMount(pub(crate) Option<DesktopPlace>);

type StatusKey = (String, EditorInstanceId, EditorRegion);

thread_local! {
    static DRAGGING: Cell<Option<Signal<bool>>> = const { Cell::new(None) };
    static STATUSES: RefCell<HashMap<StatusKey, BTreeMap<u64, Vec<HostChildStatus>>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn install() {
    DRAGGING.set(Some(create_signal(false)));
}

fn dragging() -> Signal<bool> {
    DRAGGING
        .get()
        .expect("the compositor is installed while the document is built")
}

pub(crate) fn mount() -> Option<DesktopPlace> {
    let mount = use_context::<DesktopMount>().and_then(|DesktopMount(mount)| mount);
    provide_context(DesktopMount(None));
    mount
}

impl DesktopPlace {
    pub(crate) fn offset(self) -> Memo<Rect> {
        create_memo(move || {
            let desktop = self.desktop.get();
            desktop.translate(-self.display.get().min.to_vec2())
        })
    }

    pub(crate) fn shows(self, child: Rect) -> bool {
        let display = self.display.get();
        let child = child.translate(display.min.to_vec2());
        let displays = self.displays.get();
        let owner = crate::display::owner(&displays, child);
        let owned = owner.is_some_and(|owner| displays.get(owner) == Some(&display));
        owned || (dragging().get() && display.intersects(child))
    }
}

pub(crate) fn follow_presses(input: &ForwardedInput) {
    for event in &input.events {
        let Forwarded::Pointer { event, .. } = event else {
            continue;
        };
        let held = match event.phase {
            PointerPhase::Down(_) => true,
            PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => false,
            _ => continue,
        };
        let dragging = dragging();
        if dragging.get_untracked() != held {
            dragging.set(held);
        }
    }
}

pub(crate) fn merged_statuses(
    key: StatusKey,
    mount: u64,
    statuses: Option<Vec<HostChildStatus>>,
) -> Vec<HostChildStatus> {
    STATUSES.with(|held| {
        let mut held = held.borrow_mut();
        let mounts = held.entry(key.clone()).or_default();
        match statuses {
            Some(statuses) => {
                mounts.insert(mount, statuses);
            }
            None => {
                mounts.remove(&mount);
            }
        }
        let mut merged: Vec<HostChildStatus> = Vec::new();
        for status in mounts.values().flatten() {
            if !merged.iter().any(|known| known.child == status.child) {
                merged.push(status.clone());
            }
        }
        if mounts.is_empty() {
            held.remove(&key);
        }
        merged
    })
}
