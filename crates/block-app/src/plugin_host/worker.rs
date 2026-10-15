use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::backend::{Deadline, StepTime};

pub(super) const STOPPED: &str = "The plugin worker stopped.";
pub(super) const NOT_RESPONDING: &str = "The plugin stopped responding.";
pub(super) const STEP_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) trait Presented {
    fn surface(&self) -> u32;
}

impl Presented for std::convert::Infallible {
    fn surface(&self) -> u32 {
        match *self {}
    }
}

pub(super) struct Produced<P> {
    pub(super) outbound: Vec<Vec<u8>>,
    pub(super) presented: Vec<P>,
    pub(super) painted: bool,
    pub(super) took: StepTime,
}

impl<P> Produced<P> {
    fn is_empty(&self) -> bool {
        self.outbound.is_empty() && !self.painted
    }
}

pub(super) enum Event<P> {
    Ready(Produced<P>),
    Stepped(Produced<P>),
    Failed(String),
}

impl<P> Event<P> {
    pub(super) fn wakes(&self, woken: bool, waiting: &AtomicBool) -> bool {
        let quiet = !woken && matches!(self, Self::Stepped(produced) if produced.is_empty());
        !quiet || waiting.load(Ordering::SeqCst)
    }
}

pub(super) enum Received<P> {
    Event(Event<P>),
    Empty,
    Stopped,
}

pub(super) trait Channel {
    type Presented: Presented;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool;

    fn receive(&mut self) -> Received<Self::Presented>;

    fn wait(&mut self, _timeout: Duration) -> Option<Received<Self::Presented>> {
        None
    }
}

pub(super) struct Worker<C: Channel> {
    pub(super) channel: C,
    pending: Vec<Vec<u8>>,
    waiting: Arc<AtomicBool>,
    received: Vec<Vec<u8>>,
    ready: bool,
    stepping: bool,
    carrying: bool,
    presented: Vec<C::Presented>,
    presents: u64,
    took: Option<StepTime>,
    steps: u64,
    watched: Option<(u64, Duration)>,
}

impl<C: Channel> Worker<C> {
    pub(super) fn new(channel: C, waiting: Arc<AtomicBool>) -> Self {
        Self {
            channel,
            pending: Vec::new(),
            waiting,
            received: Vec::new(),
            ready: false,
            stepping: false,
            carrying: false,
            presented: Vec::new(),
            presents: 0,
            took: None,
            steps: 0,
            watched: None,
        }
    }

    pub(super) fn ready(&self) -> bool {
        self.ready
    }

    pub(super) fn queue(&mut self, frames: Vec<Vec<u8>>) -> Option<String> {
        self.pending.extend(frames);
        if self.pending.is_empty() {
            return None;
        }
        self.waiting.store(true, Ordering::SeqCst);
        self.poll()
    }

    pub(super) fn poll(&mut self) -> Option<String> {
        loop {
            match self.channel.receive() {
                Received::Event(event) => {
                    if let Some(failure) = self.handle(event) {
                        return Some(failure);
                    }
                }
                Received::Empty => break,
                Received::Stopped => return Some(STOPPED.to_owned()),
            }
        }
        if !self.ready || self.stepping {
            return None;
        }
        self.waiting.store(false, Ordering::SeqCst);
        let carrying = !self.pending.is_empty();
        if !self.channel.step(std::mem::take(&mut self.pending)) {
            return Some(STOPPED.to_owned());
        }
        self.stepping = true;
        self.steps += 1;
        self.carrying = carrying;
        None
    }

    pub(super) fn wait(&mut self, deadline: Deadline) -> Result<bool, String> {
        let Some(timeout) = deadline.remaining() else {
            return Ok(false);
        };
        match self.channel.wait(timeout) {
            None | Some(Received::Empty) => Ok(false),
            Some(Received::Stopped) => Err(STOPPED.to_owned()),
            Some(Received::Event(event)) => match self.handle(event).or_else(|| self.poll()) {
                Some(failure) => Err(failure),
                None => Ok(true),
            },
        }
    }

    pub(super) fn hung(&mut self, now: Duration) -> Result<Option<Duration>, String> {
        if !self.stepping {
            self.watched = None;
            return Ok(None);
        }
        let since = match self.watched {
            Some((step, since)) if step == self.steps => since,
            _ => {
                self.watched = Some((self.steps, now));
                now
            }
        };
        match STEP_TIMEOUT.checked_sub(now.saturating_sub(since)) {
            Some(left) if !left.is_zero() => Ok(Some(left)),
            _ => Err(NOT_RESPONDING.to_owned()),
        }
    }

    pub(super) fn settled(&self) -> bool {
        !self.ready || (self.pending.is_empty() && !(self.stepping && self.carrying))
    }

    pub(super) fn take_received(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.received)
    }

    pub(super) fn take_presented(&mut self) -> Option<(Vec<C::Presented>, u64)> {
        if self.presented.is_empty() {
            return None;
        }
        Some((std::mem::take(&mut self.presented), self.presents))
    }

    pub(super) fn take_took(&mut self) -> Option<StepTime> {
        self.took.take()
    }

    fn handle(&mut self, event: Event<C::Presented>) -> Option<String> {
        match event {
            Event::Ready(produced) => {
                self.ready = true;
                self.absorb(produced);
            }
            Event::Stepped(produced) => {
                self.stepping = false;
                self.absorb(produced);
            }
            Event::Failed(error) => return Some(error),
        }
        None
    }

    fn absorb(&mut self, produced: Produced<C::Presented>) {
        self.received.extend(produced.outbound);
        for presented in produced.presented {
            self.presented
                .retain(|held| held.surface() != presented.surface());
            self.presented.push(presented);
        }
        if produced.painted {
            self.took = Some(produced.took);
            self.presents += 1;
        }
    }
}

#[cfg(test)]
mod tests;
