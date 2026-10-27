use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::input::Key;
use crate::node::NodeId;
use crate::window::WindowId;

pub use beui_types::input::KeyPhase;
pub use beui_types::keys::{KeyAt, KeyEvent, KeyHandler, KeyInput, KeyTarget};

struct Listener {
    at: KeyAt,
    phase: KeyPhase,
    reveal: bool,
    handler: Weak<KeyHandler>,
    window: Option<WindowId>,
}

#[derive(Default)]
pub struct KeyListeners {
    listeners: RefCell<Vec<Listener>>,
    pub(crate) held: Vec<(Key, Rc<KeyHandler>, bool)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    Host,
    Window,
}

pub(crate) struct Placed {
    pub handler: Rc<KeyHandler>,
    pub reveal: bool,
}

pub(crate) struct AtNode {
    pub node: NodeId,
    pub placed: Placed,
}

impl KeyListeners {
    pub fn listen(
        &self,
        at: KeyAt,
        phase: KeyPhase,
        reveal: bool,
        handler: Weak<KeyHandler>,
        window: Option<WindowId>,
    ) {
        self.listeners.borrow_mut().push(Listener {
            at,
            phase,
            reveal,
            handler,
            window,
        });
    }

    fn live(&self) -> std::cell::RefMut<'_, Vec<Listener>> {
        let mut listeners = self.listeners.borrow_mut();
        listeners.retain(|listener| listener.handler.strong_count() > 0);
        listeners
    }

    pub(crate) fn at(
        &self,
        stop: Stop,
        phase: KeyPhase,
        window: Option<WindowId>,
        host: &dyn Fn(WindowId) -> WindowId,
    ) -> Vec<Placed> {
        self.live()
            .iter()
            .filter(|listener| listener.phase == phase)
            .filter(|listener| match (listener.window, window) {
                (Some(own), Some(window)) => host(own) == window,
                _ => true,
            })
            .filter(|listener| {
                matches!(
                    (&listener.at, stop),
                    (KeyAt::Host, Stop::Host) | (KeyAt::Window, Stop::Window)
                )
            })
            .filter_map(|listener| {
                Some(Placed {
                    handler: listener.handler.upgrade()?,
                    reveal: listener.reveal,
                })
            })
            .collect()
    }

    pub(crate) fn placed(&self, phase: KeyPhase, root: Option<NodeId>) -> Vec<AtNode> {
        self.live()
            .iter()
            .filter(|listener| listener.phase == phase)
            .filter_map(|listener| {
                Some(AtNode {
                    node: listener.at.node(root)?,
                    placed: Placed {
                        handler: listener.handler.upgrade()?,
                        reveal: listener.reveal,
                    },
                })
            })
            .collect()
    }
}
