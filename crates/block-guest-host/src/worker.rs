use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepTime {
    pub step: Duration,
    pub gpu: Duration,
    pub submit: Duration,
}

#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    at: Instant,
}

impl Deadline {
    pub fn after(budget: Duration) -> Self {
        Self {
            at: Instant::now() + budget,
        }
    }

    pub fn at(at: Instant) -> Self {
        Self { at }
    }

    pub fn remaining(self) -> Option<Duration> {
        self.at.checked_duration_since(Instant::now())
    }
}

pub const STOPPED: &str = "The plugin worker stopped.";
pub const NOT_RESPONDING: &str = "The plugin stopped responding.";
pub const STEP_TIMEOUT: Duration = Duration::from_secs(10);

pub trait Presented {
    fn surface(&self) -> u32;
}

impl Presented for std::convert::Infallible {
    fn surface(&self) -> u32 {
        match *self {}
    }
}

pub struct Produced<P> {
    pub outbound: Vec<Vec<u8>>,
    pub presented: Vec<P>,
    pub painted: bool,
    pub took: StepTime,
}

impl<P> Produced<P> {
    pub fn sent(outbound: Vec<Vec<u8>>, took: StepTime) -> Self {
        Self {
            outbound,
            presented: Vec::new(),
            painted: false,
            took,
        }
    }

    fn is_empty(&self) -> bool {
        self.outbound.is_empty() && !self.painted
    }
}

pub enum Event<P> {
    Ready(Produced<P>),
    Stepped(Produced<P>),
    Failed(String),
}

impl<P> Event<P> {
    pub fn quiet(&self, woken: bool) -> bool {
        !woken && matches!(self, Self::Stepped(produced) if produced.is_empty())
    }
}

pub fn wakes(quiet: bool, waiting: &AtomicBool) -> bool {
    !quiet || waiting.load(Ordering::SeqCst)
}

pub enum Received<P> {
    Event(Event<P>),
    Empty,
    Stopped,
}

pub trait Channel {
    type Presented: Presented;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool;

    fn receive(&mut self) -> Received<Self::Presented>;

    fn take_woken(&mut self) -> bool;

    fn wait(&mut self, _timeout: Duration) -> Option<Received<Self::Presented>> {
        None
    }
}

impl<C: Channel + ?Sized> Channel for Box<C> {
    type Presented = C::Presented;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        (**self).step(frames)
    }

    fn receive(&mut self) -> Received<Self::Presented> {
        (**self).receive()
    }

    fn take_woken(&mut self) -> bool {
        (**self).take_woken()
    }

    fn wait(&mut self, timeout: Duration) -> Option<Received<Self::Presented>> {
        (**self).wait(timeout)
    }
}

pub struct Worker<C: Channel> {
    pub channel: C,
    pending: Vec<Vec<u8>>,
    waiting: Arc<AtomicBool>,
    received: Vec<Vec<u8>>,
    ready: bool,
    stepping: bool,
    carrying: bool,
    presented: Vec<C::Presented>,
    presents: u64,
    took: Option<StepTime>,
    stepped: (u64, Duration),
    steps: u64,
    watched: Option<(u64, Duration)>,
}

impl<C: Channel> Worker<C> {
    pub fn new(channel: C, waiting: Arc<AtomicBool>) -> Self {
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
            stepped: (0, Duration::ZERO),
            steps: 0,
            watched: None,
        }
    }

    pub fn ready(&self) -> bool {
        self.ready
    }

    pub fn queue(&mut self, frames: Vec<Vec<u8>>) -> Option<String> {
        self.pending.extend(frames);
        if self.pending.is_empty() {
            return None;
        }
        self.waiting.store(true, Ordering::SeqCst);
        self.poll()
    }

    pub fn poll(&mut self) -> Option<String> {
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
        if !self.channel.take_woken() && self.pending.is_empty() {
            return None;
        }
        let carrying = !self.pending.is_empty();
        self.waiting.store(carrying, Ordering::SeqCst);
        if !self.channel.step(std::mem::take(&mut self.pending)) {
            return Some(STOPPED.to_owned());
        }
        self.stepping = true;
        self.steps += 1;
        self.carrying = carrying;
        None
    }

    pub fn wait(&mut self, deadline: Deadline) -> Result<bool, String> {
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

    pub fn hung(&mut self, now: Duration) -> Result<Option<Duration>, String> {
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

    pub fn settled(&self) -> bool {
        !self.ready || (self.pending.is_empty() && !(self.stepping && self.carrying))
    }

    pub fn take_received(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.received)
    }

    pub fn take_presented(&mut self) -> Option<(Vec<C::Presented>, u64)> {
        if self.presented.is_empty() {
            return None;
        }
        Some((std::mem::take(&mut self.presented), self.presents))
    }

    pub fn take_took(&mut self) -> Option<StepTime> {
        self.took.take()
    }

    pub fn take_stepped(&mut self) -> (u64, Duration) {
        std::mem::replace(&mut self.stepped, (0, Duration::ZERO))
    }

    fn handle(&mut self, event: Event<C::Presented>) -> Option<String> {
        match event {
            Event::Ready(produced) => {
                self.ready = true;
                self.absorb(produced);
            }
            Event::Stepped(produced) => {
                self.stepping = false;
                self.stepped.0 += 1;
                self.stepped.1 = self.stepped.1.max(produced.took.step);
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
