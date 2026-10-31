use crate::input::{Key, KeyPhase, KeyPress};
use crate::node::{NodeId, NodeRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyInput {
    Press(KeyPress),
    Tap(Key),
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
    Control(NodeRef),
}

impl KeyAt {
    pub fn node(&self, root: Option<NodeId>) -> Option<NodeId> {
        match self {
            KeyAt::Root => root,
            KeyAt::Node(node) | KeyAt::Control(node) => node.try_get(),
            KeyAt::Host | KeyAt::Window => None,
        }
    }
}

pub type KeyHandler = dyn Fn(KeyEvent) -> bool;
