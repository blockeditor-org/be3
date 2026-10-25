use crate::ops::{Connection, OpId, Reply};

pub struct Watch<T> {
    open: Option<OpId>,
    waiting: Option<Reply<T>>,
    held: Option<T>,
}

impl<T> Default for Watch<T> {
    fn default() -> Self {
        Self {
            open: None,
            waiting: None,
            held: None,
        }
    }
}

impl<T: serde::Serialize> Watch<T> {
    pub fn change(&mut self, connection: &mut Connection, change: impl FnOnce(Option<T>) -> T) {
        let changed = change(self.held.take());
        match self.waiting.take() {
            Some(reply) => reply.send(connection, &changed),
            None => self.held = Some(changed),
        }
    }

    pub fn next(&mut self, connection: &mut Connection, reply: Reply<T>) {
        match self.held.take() {
            Some(held) => reply.send(connection, &held),
            None => self.waiting = Some(reply),
        }
    }

    pub fn opened(&mut self, op: OpId) {
        self.open = Some(op);
        self.waiting = None;
    }

    pub fn closed(&mut self, op: OpId) {
        if self.open == Some(op) {
            *self = Self::default();
        }
    }
}

impl<T> Watch<T> {
    pub fn is(&self, op: OpId) -> bool {
        self.open == Some(op)
    }
}
