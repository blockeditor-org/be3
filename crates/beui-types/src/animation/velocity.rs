use std::collections::VecDeque;
use std::time::Instant;

use geometry::{Pos2, Vec2};

use crate::input::{MAX_TOUCH_VELOCITY, TOUCH_VELOCITY_WINDOW};

const MIN_SPAN: f32 = 0.004;

#[derive(Clone, Debug, Default)]
pub struct Velocity {
    samples: VecDeque<(Instant, Pos2)>,
    velocity: Vec2,
}

impl Velocity {
    pub fn new(at: Instant, pos: Pos2) -> Self {
        Self {
            samples: VecDeque::from([(at, pos)]),
            velocity: Vec2::ZERO,
        }
    }

    pub fn sample(&mut self, now: Instant, pos: Pos2) {
        self.samples.push_back((now, pos));
        while self.samples.len() > 2
            && self.samples.front().is_some_and(|(when, _)| {
                now.duration_since(*when).as_secs_f32() > TOUCH_VELOCITY_WINDOW
            })
        {
            self.samples.pop_front();
        }
        let Some((then, origin)) = self.samples.front().copied() else {
            return;
        };
        let elapsed = now.duration_since(then).as_secs_f32();
        if elapsed < MIN_SPAN {
            return;
        }
        let measured = (pos - origin) * elapsed.recip();
        let speed = measured.x.hypot(measured.y);
        self.velocity = match speed > MAX_TOUCH_VELOCITY {
            true => measured * (MAX_TOUCH_VELOCITY / speed),
            false => measured,
        };
    }

    pub fn get(&self) -> Vec2 {
        self.velocity
    }
}
