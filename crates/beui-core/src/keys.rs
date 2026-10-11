use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::callback::NodeRef;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyInput {
    Press(KeyPress),
    Tap(Key),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Capture,
    Bubble,
    Held,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyTarget {
    pub node: Option<NodeId>,
    pub typing: bool,
    pub app: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub input: KeyInput,
    pub phase: KeyPhase,
    pub target: KeyTarget,
}

impl KeyEvent {
    pub fn key(&self) -> Key {
        match self.input {
            KeyInput::Press(press) => press.key,
            KeyInput::Tap(key) => key,
        }
    }

    pub fn press(&self) -> Option<KeyPress> {
        match self.input {
            KeyInput::Press(press) => Some(press),
            KeyInput::Tap(_) => None,
        }
    }

    pub fn tap(&self) -> Option<Key> {
        match self.input {
            KeyInput::Tap(key) => Some(key),
            KeyInput::Press(_) => None,
        }
    }

    pub fn held(&self) -> bool {
        self.phase == KeyPhase::Held
    }
}

#[derive(Clone)]
pub enum KeyAt {
    Host,
    Window,
    Root,
    Node(NodeRef),
}

impl KeyAt {
    pub(crate) fn node(&self, root: Option<NodeId>) -> Option<NodeId> {
        match self {
            KeyAt::Root => root,
            KeyAt::Node(node) => node.try_get(),
            KeyAt::Host | KeyAt::Window => None,
        }
    }
}

pub type KeyHandler = dyn Fn(KeyEvent) -> bool;

struct Listener {
    at: KeyAt,
    phase: KeyPhase,
    handler: Weak<KeyHandler>,
}

pub(crate) enum Holder {
    Listener(Rc<KeyHandler>),
    Cycle,
}

#[derive(Default)]
pub struct KeyListeners {
    listeners: RefCell<Vec<Listener>>,
    pub(crate) held: Vec<(Key, Holder)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    Host,
    Window,
    Node(NodeId),
}

impl KeyListeners {
    pub fn listen(&self, at: KeyAt, phase: KeyPhase, handler: Weak<KeyHandler>) {
        self.listeners
            .borrow_mut()
            .push(Listener { at, phase, handler });
    }

    pub(crate) fn at(
        &self,
        stop: Stop,
        phase: KeyPhase,
        root: Option<NodeId>,
    ) -> Vec<Rc<KeyHandler>> {
        let mut listeners = self.listeners.borrow_mut();
        listeners.retain(|listener| listener.handler.strong_count() > 0);
        listeners
            .iter()
            .filter(|listener| listener.phase == phase)
            .filter(|listener| match (&listener.at, stop) {
                (KeyAt::Host, Stop::Host) | (KeyAt::Window, Stop::Window) => true,
                (at, Stop::Node(node)) => at.node(root) == Some(node),
                _ => false,
            })
            .filter_map(|listener| listener.handler.upgrade())
            .collect()
    }

    pub(crate) fn placed(
        &self,
        phase: KeyPhase,
        root: Option<NodeId>,
    ) -> Vec<(NodeId, Rc<KeyHandler>)> {
        let mut listeners = self.listeners.borrow_mut();
        listeners.retain(|listener| listener.handler.strong_count() > 0);
        listeners
            .iter()
            .filter(|listener| listener.phase == phase)
            .filter_map(|listener| Some((listener.at.node(root)?, listener.handler.upgrade()?)))
            .collect()
    }
}
