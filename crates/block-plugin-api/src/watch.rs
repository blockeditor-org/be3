use crate::ops::{Connection, OpError, OpId, Outcome, Reply};

pub struct Watch<T> {
    open: Option<OpId>,
    waiting: Option<OpId>,
    held: Option<T>,
}

pub struct Encoded(pub Vec<u8>);

impl<T> Default for Watch<T> {
    fn default() -> Self {
        Self {
            open: None,
            waiting: None,
            held: None,
        }
    }
}

impl<T> Watch<T> {
    pub fn opened(&mut self, op: OpId) {
        self.open = Some(op);
        self.waiting = None;
    }

    pub fn closed(&mut self, op: OpId) {
        if self.open == Some(op) {
            *self = Self::default();
        }
    }

    pub fn is(&self, op: OpId) -> bool {
        self.open == Some(op)
    }

    pub fn waiting(&self) -> bool {
        self.waiting.is_some()
    }

    pub fn withdraw(&mut self, connection: &mut Connection, next: OpId) -> bool {
        if self.waiting != Some(next) {
            return false;
        }
        self.waiting = None;
        connection.complete(next, Outcome::Err(OpError::Canceled));
        true
    }

    fn wait(&mut self, connection: &mut Connection, next: OpId) {
        if let Some(earlier) = self.waiting.replace(next) {
            connection.complete(earlier, Outcome::Err(OpError::Canceled));
        }
    }

    fn answer(&mut self, connection: &mut Connection, outcome: impl FnOnce(T) -> Outcome) {
        if let (Some(next), Some(held)) = (self.waiting, self.held.take()) {
            self.waiting = None;
            connection.complete(next, outcome(held));
        }
    }
}

impl<T: serde::Serialize> Watch<T> {
    pub fn change(&mut self, connection: &mut Connection, change: impl FnOnce(Option<T>) -> T) {
        self.held = Some(change(self.held.take()));
        self.answer(connection, |value| Outcome::returned(&value));
    }

    pub fn next(&mut self, connection: &mut Connection, reply: Reply<T>) {
        self.wait(connection, reply.op());
        self.answer(connection, |value| Outcome::returned(&value));
    }
}

impl Watch<Encoded> {
    pub fn set(&mut self, connection: &mut Connection, value: Vec<u8>) {
        self.held = Some(Encoded(value));
        self.answer(connection, |Encoded(bytes)| Outcome::Ok(bytes));
    }

    pub fn next_encoded(&mut self, connection: &mut Connection, next: OpId) {
        self.wait(connection, next);
        self.answer(connection, |Encoded(bytes)| Outcome::Ok(bytes));
    }

    pub fn end(&mut self, connection: &mut Connection) {
        if let Some(next) = self.waiting.take() {
            connection.complete(next, Outcome::Err(OpError::Canceled));
        }
    }
}
