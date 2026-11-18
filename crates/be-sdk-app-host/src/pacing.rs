//! Frame pacing and settling: when a host asks an app for a frame, how long it waits for the
//! answers before drawing its own, and which app waits for another's layout
//! (guides/apps/runtime.md, Frames and pacing). `Pacing`'s times are the host's frame clock,
//! which a driven host may stop; `FrameClock`'s are real time, which the host reads (a
//! `Duration` since an origin of its choosing) and interprets, so the library reads no clock.

use std::{collections::VecDeque, hash::Hash, time::Duration};

use be_lib_stable_hash::HashSet;

/// How long a frame asked for may go unanswered before the host asks again.
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(1);
/// How many frames in a row an app may say its input is still waiting (`input-waiting`) before
/// the host counts one as answering it all the same, so an app that always says so cannot hold
/// a press's answer for ever.
pub const INPUT_WAITING_FRAMES: u32 = 4;
/// How long settling waits while a driven host's clock is stopped (guides/running/drive.md), so
/// a slow machine never changes which frame shows an answer.
pub const STOPPED_CLOCK_FRAME_BUDGET: Duration = Duration::from_secs(10);
const UNKNOWN_REFRESH: Duration = Duration::from_micros(16_667);
const UNMEASURED_COMPOSITE: Duration = Duration::from_millis(4);
const SUBMIT_MARGIN: Duration = Duration::from_millis(1);
const REMEMBERED_COMPOSITES: usize = 16;

/// One app's frames: whether it needs one, when it was asked and answered, and when it wants
/// the next.
#[derive(Default)]
pub struct Pacing {
    needed: bool,
    unseen: bool,
    animating: bool,
    paint_at: Option<Duration>,
    drawn_at: Duration,
    requested_at: Option<Duration>,
    animated: u64,
    asked_in: u64,
    painting: bool,
    needed_again: bool,
    /// Frames asked of the app and answered by it, so the host can tell when the app has drawn
    /// a frame that took input it sent: the first answer to a request made after it. A frame
    /// that says it has not taken in all its input yet (`input-waiting`) does not count as
    /// answered, for up to `INPUT_WAITING_FRAMES` in a row.
    asked: u64,
    answered: u64,
    /// The frames in a row that said their input was still waiting.
    waiting: u32,
}

/// What `Pacing::ask` decides from.
#[derive(Clone, Copy, Debug)]
pub struct Asking {
    /// The app has a surface to draw to.
    pub drawing: bool,
    /// The host frame will be seen.
    pub seen: bool,
    /// The host's frame clock.
    pub now: Duration,
    /// The host frame.
    pub pass: u64,
    /// Its placer lays out first (`held_behind`).
    pub held: bool,
    /// Input sent to it is still in flight.
    pub holds_input: bool,
}

/// What `Pacing::ask` decided for this host frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// No frame is due.
    Nothing,
    /// A frame is due but waits for a later host frame: the app was asked in this one, or its
    /// placer lays out first (`held_behind`).
    Later,
    /// A frame is due once the app has taken in the input it holds.
    AfterInput,
    /// Ask the app for a frame now (`Draw`).
    Draw,
}

impl Pacing {
    /// Forgets every frame in flight, as a restart does (`Run::restart`).
    pub(crate) fn reset(&mut self) {
        self.needed = false;
        self.unseen = false;
        self.animating = false;
        self.paint_at = None;
        self.requested_at = None;
        self.painting = false;
        self.needed_again = false;
        self.answered = self.asked;
        self.waiting = 0;
    }

    fn due(&self, seen: bool, now: Duration, pass: u64) -> bool {
        (self.needed || (seen && self.unseen) || (self.animated != pass && self.asked_by(now)))
            && self
                .requested_at
                .is_none_or(|at| now.saturating_sub(at) >= FRAME_TIMEOUT)
    }

    fn asked_by(&self, now: Duration) -> bool {
        self.paint_at.is_some_and(|at| at <= now) && now > self.drawn_at
    }

    /// Whether the app would be asked for a frame in host frame `pass`, which has been seen
    /// (`seen`) or not.
    pub fn will_draw(&self, seen: bool, now: Duration, pass: u64) -> bool {
        self.asked_in != pass && self.due(seen, now, pass)
    }

    /// Whether the app was asked for a frame in host frame `pass` and has yet to lay it out.
    pub fn laying_out(&self, pass: u64) -> bool {
        self.requested_at.is_some() && self.asked_in == pass && !self.painting
    }

    /// Whether a frame is needed or in flight.
    pub fn drawing(&self) -> bool {
        self.needed || self.requested_at.is_some()
    }

    /// Whether a frame was asked for and not yet presented (until `FRAME_TIMEOUT`, past which
    /// `ask` asks again).
    pub fn in_flight(&self) -> bool {
        self.requested_at.is_some()
    }

    /// Whether a frame asked for has been laid out and not yet presented.
    pub fn painting(&self) -> bool {
        self.painting
    }

    /// Whether the app's last present said it is animating.
    pub fn animating(&self) -> bool {
        self.animating
    }

    /// The frames asked of the app and the frames it answered: input sent while the first was
    /// `n` has been drawn once the second exceeds `n`.
    pub fn answered(&self) -> (u64, u64) {
        (self.asked, self.answered)
    }

    /// Asks for a frame. One asked for while a frame is in flight is asked again once that
    /// frame is answered, which `ready` would otherwise clear along with the frame's own need.
    pub fn need(&mut self) {
        self.needed = true;
        self.needed_again |= self.requested_at.is_some();
    }

    pub fn need_if(&mut self, needed: bool) {
        if needed {
            self.need();
        }
    }

    /// Decides whether to ask an app for a frame, and records the request when it does.
    pub fn ask(&mut self, asking: Asking) -> Ask {
        let Asking {
            drawing,
            seen,
            now,
            pass,
            held,
            holds_input,
        } = asking;
        self.need_if(seen && self.unseen);
        let due = drawing && self.due(false, now, pass);
        if !due {
            Ask::Nothing
        } else if held || self.asked_in == pass {
            Ask::Later
        } else if holds_input {
            Ask::AfterInput
        } else {
            self.request(now, pass);
            self.unseen = !seen;
            Ask::Draw
        }
    }

    fn request(&mut self, now: Duration, pass: u64) {
        self.asked += 1;
        self.requested_at = Some(now);
        self.animated = pass;
        self.asked_in = pass;
    }

    /// The app sent the layout of the frame asked for.
    pub fn laid_out(&mut self) {
        self.painting = self.requested_at.is_some();
    }

    /// The app presented a frame: it is animating or not, wants the next after `repaint_after`
    /// if at all, and has or has not taken in all its input (`input_waiting`).
    pub fn presented(
        &mut self,
        now: Duration,
        animating: bool,
        repaint_after: Option<Duration>,
        input_waiting: bool,
    ) {
        self.animating = animating;
        self.painting = false;
        match input_waiting && self.waiting < INPUT_WAITING_FRAMES {
            true => self.waiting += 1,
            false => {
                self.answered = self.asked;
                self.waiting = 0;
            }
        }
        self.needed = std::mem::take(&mut self.needed_again);
        self.requested_at = None;
        self.drawn_at = now;
        self.paint_at = repaint_after.map(|delay| now + delay);
    }

    /// How long until the host should look again: a frame in flight times out, and an app that
    /// draws wants its next frame when it said.
    pub fn wake_after(&self, now: Duration, drawing: bool) -> Option<Duration> {
        match (self.requested_at, self.paint_at) {
            (Some(_), _) => Some(FRAME_TIMEOUT),
            (None, Some(at)) if drawing => Some(at.saturating_sub(now)),
            _ => None,
        }
    }
}

/// The host's frame deadline, the display's next present less the time the host's own work
/// after settling has been taking, and settling against it.
#[derive(Default)]
pub struct FrameClock {
    present: Option<(Duration, Duration)>,
    settled_at: Option<Duration>,
    composites: VecDeque<Duration>,
    deadline: Option<(u64, Duration)>,
    waited: Duration,
    over_budget: u64,
}

impl FrameClock {
    /// A host frame begins: the display presents next at `next_present` every `refresh`, and
    /// the last frame was ready to present at `prepared`.
    pub fn begin_frame(
        &mut self,
        next_present: Option<Duration>,
        refresh: Option<Duration>,
        prepared: Option<Duration>,
    ) {
        if let (Some(settled), Some(prepared)) = (self.settled_at.take(), prepared)
            && prepared > settled
        {
            if self.composites.len() == REMEMBERED_COMPOSITES {
                self.composites.pop_front();
            }
            self.composites.push_back(prepared - settled);
        }
        self.present = next_present.map(|at| (at, refresh.unwrap_or(UNKNOWN_REFRESH)));
    }

    /// When settling in host frame `pass` must end, read at `now`: the same for every settle of
    /// one frame, and with the host's clock stopped, `STOPPED_CLOCK_FRAME_BUDGET` after the first.
    pub fn deadline(&mut self, pass: u64, clock_stopped: bool, now: Duration) -> Duration {
        let deadline = match self.deadline {
            Some((at, deadline)) if at == pass => deadline,
            _ => self.next_deadline(clock_stopped, now),
        };
        self.deadline = Some((pass, deadline));
        deadline
    }

    fn next_deadline(&self, clock_stopped: bool, now: Duration) -> Duration {
        if clock_stopped {
            return now + STOPPED_CLOCK_FRAME_BUDGET;
        }
        let origin = self.present.map_or(now, |(at, _)| at.min(now));
        let present = self.present.map(|(at, interval)| (at - origin, interval));
        now + frame_wait(now - origin, present, &self.composites)
    }

    /// Waits for every asked app's answer at once, until `deadline` (`settle`); true when an
    /// answer changes what the host lays out. What follows until the frame is prepared is the
    /// host's own work.
    pub fn settle(&mut self, apps: &mut impl Settling, deadline: Duration) -> bool {
        let started = apps.now();
        let settled = settle(apps, deadline);
        let ended = apps.now();
        self.waited += ended.saturating_sub(started);
        self.over_budget += u64::from(settled == Settled::OutOfTime);
        self.settled_at = Some(ended);
        settled == Settled::Relaid
    }

    /// The time spent waiting on apps since last taken, and the frames that ran out of time.
    pub fn take_waited(&mut self) -> (Duration, u64) {
        (std::mem::take(&mut self.waited), self.over_budget)
    }
}

fn frame_wait(
    now: Duration,
    present: Option<(Duration, Duration)>,
    composites: &VecDeque<Duration>,
) -> Duration {
    let composite = composites
        .iter()
        .max()
        .copied()
        .unwrap_or(UNMEASURED_COMPOSITE)
        + SUBMIT_MARGIN;
    let (mut present, interval) = present.unwrap_or((now + UNKNOWN_REFRESH, UNKNOWN_REFRESH));
    let interval = interval.max(Duration::from_millis(1));
    while present < now + composite {
        present += interval;
    }
    present - composite - now
}

/// Which apps wait for another to lay out first, from each `(key, child app, placer app)` of a
/// child an active app places in another (or the other way round, where the placer fits to its
/// child), ordered by `key`: the second of each pair, unless the pair closes a cycle with those
/// before it, so two apps placing each other are never both held.
pub fn held_behind<K: Ord, I: Ord + Hash + Clone>(mut edges: Vec<(K, I, I)>) -> HashSet<I> {
    edges.sort();
    edges.dedup();
    let mut kept: Vec<(I, I)> = Vec::new();
    for (_, first, then) in edges {
        if !reaches(&kept, &then, &first) {
            kept.push((first, then));
        }
    }
    kept.into_iter().map(|(_, then)| then).collect()
}

fn reaches<I: Eq + Hash>(edges: &[(I, I)], from: &I, to: &I) -> bool {
    let mut seen = HashSet::default();
    let mut next = vec![from];
    while let Some(at) = next.pop() {
        if at == to {
            return true;
        }
        if seen.insert(at) {
            next.extend(
                edges
                    .iter()
                    .filter(|(placer, _)| placer == at)
                    .map(|(_, placed)| placed),
            );
        }
    }
    false
}

/// The apps a host settles: what `settle` steps and waits on.
pub trait Settling {
    /// Takes in what the asked apps sent; true when an answer changes what the host lays out.
    fn pump(&mut self) -> bool;
    /// Asks the apps for the frames now due.
    fn request_frames(&mut self);
    /// Whether an asked app still owes an answer.
    fn outstanding(&self) -> bool;
    /// Waits until an app may have sent something since `rung` (the bell's count before the
    /// last `pump`); false once `deadline` (real time, as `now` reads it) passed.
    fn wait(&mut self, rung: u64, deadline: Duration) -> bool;
    /// The bell's count, read before pumping.
    fn rung(&self) -> u64;
    /// Real time, since an origin of the host's choosing.
    fn now(&self) -> Duration;
}

/// How settling ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settled {
    /// Every asked app answered.
    Answered,
    /// An answer changes what the host lays out: lay out and settle again.
    Relaid,
    /// The deadline passed with answers outstanding.
    OutOfTime,
}

/// Waits for every asked app's answer at once, until `deadline`.
pub fn settle(apps: &mut impl Settling, deadline: Duration) -> Settled {
    loop {
        let rung = apps.rung();
        if apps.pump() {
            return Settled::Relaid;
        }
        apps.request_frames();
        if !apps.outstanding() {
            return Settled::Answered;
        }
        if !apps.wait(rung, deadline) {
            return Settled::OutOfTime;
        }
    }
}

#[cfg(test)]
mod tests;
