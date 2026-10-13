use super::lock::{Lock, Trigger};

pub(crate) trait Inhibit {
    fn take(&mut self);
    fn release(&mut self);
}

#[derive(Debug, Default)]
pub(crate) struct SleepGuard {
    held: bool,
    releasing: bool,
}

impl SleepGuard {
    pub(crate) fn hold(&mut self, inhibit: &mut impl Inhibit) {
        if !self.held {
            inhibit.take();
            self.held = true;
        }
    }

    pub(crate) fn prepare(&mut self, starting: bool, lock: &mut Lock, inhibit: &mut impl Inhibit) {
        if starting {
            lock.lock(Trigger::Sleep);
            self.releasing = true;
        } else {
            self.releasing = false;
            self.hold(inhibit);
        }
    }

    pub(crate) fn waiting(&self) -> bool {
        self.releasing && self.held
    }

    pub(crate) fn shown(&mut self, inhibit: &mut impl Inhibit) {
        if !self.waiting() {
            return;
        }
        self.releasing = false;
        self.held = false;
        inhibit.release();
    }
}

#[cfg(test)]
mod tests;
