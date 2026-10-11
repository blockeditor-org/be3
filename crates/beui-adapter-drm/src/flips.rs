use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flip {
    pub screen: String,
    pub time: Duration,
    pub refresh: Duration,
    pub sequence: u64,
    pub hardware_clock: bool,
}

#[derive(Default)]
struct Shown {
    flips: Vec<Flip>,
    watched: bool,
}

#[derive(Clone, Default)]
pub struct Flips(Rc<RefCell<Shown>>);

impl Flips {
    pub fn take(&self) -> Vec<Flip> {
        std::mem::take(&mut self.0.borrow_mut().flips)
    }

    pub fn watch(&self, watched: bool) {
        self.0.borrow_mut().watched = watched;
    }

    pub(crate) fn drawn(&self, screen: &str) {
        self.0
            .borrow_mut()
            .flips
            .retain(|held| held.screen != screen);
    }

    pub(crate) fn shown(&self, flip: Flip) -> bool {
        let mut shown = self.0.borrow_mut();
        shown.flips.retain(|held| held.screen != flip.screen);
        shown.flips.push(flip);
        shown.watched
    }
}

#[cfg(test)]
mod tests;
