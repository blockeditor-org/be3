use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_plugin_api::{
    BarAction, BlockPick, ChildId, ChildPreview, CreationProgress, DialogStatus,
    EditorCapabilities, InteractionMode, MenuEntry, ResizeMode,
};
use geometry::Vec2;

use crate::EditorHost;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildState {
    pub placed: bool,
    pub available: bool,
    pub active: bool,
    pub intrinsic_size: Option<Vec2>,
    pub aspect_ratio: Option<f32>,
    pub interaction: Option<InteractionMode>,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
    pub child: Option<ChildId>,
    pub menu: Vec<MenuEntry>,
    pub creation: Option<CreationProgress>,
    pub preview: ChildPreview,
    pub dialog: DialogStatus,
    pub picked: Option<BlockPick>,
    pub card: bool,
}

impl ChildState {
    pub fn of(host: &EditorHost, child: Option<ChildId>) -> Self {
        let Some(status) = child.and_then(|child| host.child_status(child)) else {
            return Self {
                placed: child.is_some(),
                child,
                ..Self::default()
            };
        };
        Self {
            placed: true,
            available: status.available,
            active: status.active,
            intrinsic_size: status
                .intrinsic
                .map(|size| Vec2::new(size.width, size.height)),
            aspect_ratio: status.aspect_ratio,
            interaction: Some(status.interaction),
            capabilities: status.capabilities,
            resize: status.resize,
            error: status.error,
            child,
            menu: MenuEntry::nest(status.menu).unwrap_or_default(),
            creation: status.creation,
            preview: status.preview,
            dialog: status.dialog,
            picked: status.picked,
            card: status.card,
        }
    }
}

struct Slot {
    child: Cell<Option<ChildId>>,
    state: RefCell<ChildState>,
    on_state: Box<dyn Fn(&ChildState)>,
    on_bar: Box<dyn Fn(BarAction)>,
    on_focus: Box<dyn Fn()>,
}

type Slots = RefCell<Vec<(u64, Rc<Slot>)>>;

#[derive(Default)]
pub(super) struct Children {
    slots: Rc<Slots>,
    next: Cell<u64>,
    moved: Rc<Cell<bool>>,
}

impl Children {
    pub(super) fn add(
        &self,
        on_state: impl Fn(&ChildState) + 'static,
        on_bar: impl Fn(BarAction) + 'static,
        on_focus: impl Fn() + 'static,
    ) -> PlacedChild {
        let key = self.next.get();
        self.next.set(key + 1);
        let slot = Rc::new(Slot {
            child: Cell::new(None),
            state: RefCell::default(),
            on_state: Box::new(on_state),
            on_bar: Box::new(on_bar),
            on_focus: Box::new(on_focus),
        });
        self.slots.borrow_mut().push((key, Rc::clone(&slot)));
        PlacedChild {
            key,
            slot,
            slots: Rc::downgrade(&self.slots),
            moved: Rc::clone(&self.moved),
        }
    }

    pub(super) fn take_moved(&self) -> bool {
        self.moved.take()
    }

    pub(super) fn follow(&self, host: &EditorHost) {
        let slots: Vec<Rc<Slot>> = self
            .slots
            .borrow()
            .iter()
            .map(|(_, slot)| Rc::clone(slot))
            .collect();
        for slot in slots {
            if let Some(child) = slot.child.get() {
                for action in host.take_child_bar_actions(child) {
                    (slot.on_bar)(action);
                }
                if host.take_child_focus(child) {
                    (slot.on_focus)();
                }
            }
            let state = ChildState::of(host, slot.child.get());
            if *slot.state.borrow() == state {
                continue;
            }
            *slot.state.borrow_mut() = state.clone();
            (slot.on_state)(&state);
        }
    }
}

pub struct PlacedChild {
    key: u64,
    slot: Rc<Slot>,
    slots: std::rc::Weak<Slots>,
    moved: Rc<Cell<bool>>,
}

impl PlacedChild {
    pub fn child(&self) -> Option<ChildId> {
        self.slot.child.get()
    }

    pub fn state(&self) -> ChildState {
        self.slot.state.borrow().clone()
    }

    pub fn place(&self, child: Option<ChildId>) {
        if self.slot.child.replace(child) != child {
            self.moved.set(true);
        }
    }
}

impl Drop for PlacedChild {
    fn drop(&mut self) {
        if let Some(slots) = self.slots.upgrade() {
            slots.borrow_mut().retain(|(key, _)| *key != self.key);
        }
    }
}
