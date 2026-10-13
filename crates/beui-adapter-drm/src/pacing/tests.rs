use super::*;

mod a_continuous_animation_draws_one_frame_per_flip;
mod a_frame_that_shows_nothing_waits_for_the_next_refresh;
mod an_idle_output_draws_nothing;
mod outputs_at_different_rates_each_flip_at_their_own;

const SECOND: Duration = Duration::from_secs(1);
const MOST_FRAMES: u32 = 10_000;

struct Display {
    refresh: Duration,
    flips_at: Option<Duration>,
    flipped: bool,
    flips: u32,
    drawn: u32,
}

impl Display {
    fn new(hertz: u64) -> Self {
        Self {
            refresh: Duration::from_nanos(1_000_000_000 / hertz),
            flips_at: None,
            flipped: false,
            flips: 0,
            drawn: 0,
        }
    }

    fn lane(&self) -> Lane {
        Lane {
            busy: self.flips_at.is_some(),
            flipped: self.flipped,
            refresh: self.refresh,
        }
    }

    fn next_vblank(&self, now: Duration) -> Duration {
        let refresh = self.refresh.as_nanos();
        let ticks = now.as_nanos() / refresh + 1;
        Duration::from_nanos((ticks * refresh) as u64)
    }
}

enum App {
    Idle,
    Animating,
    Ticking,
}

struct Session {
    now: Duration,
    pacer: Pacer,
    displays: Vec<Display>,
    app: App,
    frames: u32,
    shown: bool,
}

impl Session {
    fn new(app: App, displays: Vec<Display>) -> Self {
        let mut pacer = Pacer::default();
        pacer.ask();
        Self {
            now: Duration::ZERO,
            pacer,
            displays,
            app,
            frames: 0,
            shown: false,
        }
    }

    fn lanes(&self) -> Vec<Lane> {
        self.displays.iter().map(Display::lane).collect()
    }

    fn frame(&mut self) {
        self.frames += 1;
        let changed = !self.shown || matches!(self.app, App::Animating);
        self.shown = true;
        let again = !matches!(self.app, App::Idle);
        self.pacer.ran(self.now, again, None);
        for display in &mut self.displays {
            display.flipped = false;
            if changed && display.flips_at.is_none() {
                display.drawn += 1;
                display.flips_at = Some(display.next_vblank(self.now));
            }
        }
    }

    fn run_until(&mut self, end: Duration) {
        loop {
            let wait = self.pacer.wait(self.now, &self.lanes());
            if wait == Wait::Now {
                if self.frames > MOST_FRAMES {
                    return;
                }
                self.frame();
                continue;
            }
            let flip = self
                .displays
                .iter()
                .filter_map(|display| display.flips_at)
                .min();
            let woken = match wait {
                Wait::Until(at) => Some(at),
                _ => None,
            };
            let Some(next) = flip.into_iter().chain(woken).min() else {
                return;
            };
            if next > end {
                return;
            }
            self.now = next;
            for display in &mut self.displays {
                if display.flips_at.is_some_and(|at| at <= next) {
                    display.flips_at = None;
                    display.flipped = true;
                    display.flips += 1;
                }
            }
        }
    }
}
