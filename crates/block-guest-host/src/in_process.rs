use std::{
    collections::VecDeque,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use block_guest_abi::{Flow, Guest, Waker};

use crate::{Channel, Event, Produced, Received, StepTime, Wake, panic_message, wakes};

pub struct InProcess<G: Guest> {
    guest: Option<G>,
    events: VecDeque<Event<Infallible>>,
    woken: Arc<AtomicBool>,
    taken: bool,
    waiting: Arc<AtomicBool>,
    wake: Wake,
}

impl<G: Guest> InProcess<G> {
    pub fn start(wake: Wake, waiting: Arc<AtomicBool>) -> Self {
        let woken = Arc::new(AtomicBool::new(false));
        let waker = {
            let woken = Arc::clone(&woken);
            let wake = Arc::clone(&wake);
            Waker::new(move || {
                woken.store(true, Ordering::Release);
                wake();
            })
        };
        let mut channel = Self {
            guest: None,
            events: VecDeque::new(),
            woken,
            taken: false,
            waiting,
            wake,
        };
        let mut outbound = Vec::new();
        let started = catch_unwind(AssertUnwindSafe(|| G::start(waker, &mut outbound)));
        let event = match started {
            Ok(Ok(guest)) => {
                channel.guest = Some(guest);
                Event::Ready(Produced::sent(outbound, StepTime::default()))
            }
            Ok(Err(error)) => Event::Failed(error),
            Err(panic) => Event::Failed(panic_message(&*panic)),
        };
        channel.report(event);
        channel
    }

    pub fn guest(&self) -> Option<&G> {
        self.guest.as_ref()
    }

    fn report(&mut self, event: Event<Infallible>) {
        let quiet = event.quiet(self.woken.load(Ordering::Acquire));
        self.events.push_back(event);
        if wakes(quiet, &self.waiting) {
            (self.wake)();
        }
    }
}

impl<G: Guest> Channel for InProcess<G> {
    type Presented = Infallible;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        let Some(guest) = self.guest.as_mut() else {
            self.report(Event::Stepped(Produced::sent(
                Vec::new(),
                StepTime::default(),
            )));
            return true;
        };
        let woken = std::mem::take(&mut self.taken) | self.woken.swap(false, Ordering::AcqRel);
        let mut outbound = Vec::new();
        let started = Instant::now();
        let stepped = catch_unwind(AssertUnwindSafe(|| {
            guest.step(frames, woken, &mut outbound)
        }));
        let took = StepTime {
            step: started.elapsed(),
            ..StepTime::default()
        };
        let event = match stepped {
            Ok(Ok(Flow::Running)) => Event::Stepped(Produced::sent(outbound, took)),
            Ok(Ok(Flow::Finished)) => {
                self.guest = None;
                Event::Stepped(Produced::sent(outbound, took))
            }
            Ok(Err(error)) => {
                self.guest = None;
                Event::Failed(error)
            }
            Err(panic) => {
                self.guest = None;
                Event::Failed(panic_message(&*panic))
            }
        };
        self.report(event);
        true
    }

    fn take_woken(&mut self) -> bool {
        let woken = self.woken.swap(false, Ordering::AcqRel);
        self.taken |= woken;
        woken
    }

    fn receive(&mut self) -> Received<Infallible> {
        match self.events.pop_front() {
            Some(event) => Received::Event(event),
            None => Received::Empty,
        }
    }
}
