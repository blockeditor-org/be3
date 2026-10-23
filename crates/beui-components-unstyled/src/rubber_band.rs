#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::time::Instant;

use beui_core::geometry::{Pos2, Vec2, vec2};

const RUBBER_BAND_FACTOR: f32 = 0.55;
const VELOCITY_WINDOW: f32 = 0.08;
const GLIDE_FRICTION: f32 = 9.0;
pub const MINIMUM_VELOCITY: f32 = 5.0;

pub use beui_core::animation::Spring;

pub const SCROLL_SPRING: Spring = Spring {
    stiffness: 180.0,
    damping: 26.832_815,
};

pub const WINDOW_SPRING: Spring = Spring {
    stiffness: 400.0,
    damping: 38.0,
};

fn reach(dimension: f32) -> f32 {
    dimension.max(1.0) * RUBBER_BAND_FACTOR
}

pub fn rubber_band(distance: f32, dimension: f32) -> f32 {
    let reach = reach(dimension);
    distance * reach / (distance.abs() + reach)
}

pub fn unband(overscroll: f32, dimension: f32) -> f32 {
    let reach = reach(dimension);
    let stretch = overscroll.abs().min(reach * 0.99);
    (stretch * reach / (reach - stretch)).copysign(overscroll)
}

pub fn spring_back(overscroll: &mut f32, velocity: &mut f32, elapsed: f32, spring: Spring) {
    let from = *overscroll;
    spring.advance(overscroll, velocity, elapsed);
    if *overscroll * from < 0.0 {
        *overscroll = 0.0;
        *velocity = 0.0;
    }
}

pub fn bounce(stretch: &mut f32, velocity: &mut f32, elapsed: f32) -> f32 {
    let mut elapsed = elapsed;
    if *stretch * *velocity > 0.0 {
        let frequency = SCROLL_SPRING.stiffness.sqrt();
        let peak = (velocity.abs() / frequency).atan2(stretch.abs()) / frequency;
        if elapsed < peak {
            let outwards = Spring {
                damping: 0.0,
                ..SCROLL_SPRING
            };
            outwards.advance(stretch, velocity, elapsed);
            return 0.0;
        }
        *stretch = stretch
            .abs()
            .hypot(*velocity / frequency)
            .copysign(*stretch);
        *velocity = 0.0;
        elapsed -= peak;
    }
    let from = *stretch;
    SCROLL_SPRING.advance(stretch, velocity, elapsed);
    if *stretch * from < 0.0 {
        *stretch = 0.0;
        return std::mem::take(velocity);
    }
    0.0
}

pub struct Band {
    pub offset: Vec2,
    velocity: Vec2,
    held: bool,
    stepped: Instant,
    samples: VecDeque<(Instant, Pos2)>,
    spring: Spring,
}

impl Band {
    pub fn new(spring: Spring) -> Self {
        Self {
            offset: Vec2::ZERO,
            velocity: Vec2::ZERO,
            held: false,
            stepped: beui_core::timer::now(),
            samples: VecDeque::new(),
            spring,
        }
    }

    pub fn grab(&mut self, dimensions: Vec2) -> Vec2 {
        self.held = true;
        self.velocity = Vec2::ZERO;
        self.samples.clear();
        vec2(
            unband(self.offset.x, dimensions.x),
            unband(self.offset.y, dimensions.y),
        )
    }

    pub fn stretch(
        &mut self,
        position: Pos2,
        distance: Vec2,
        dimensions: Vec2,
        banding: bool,
        now: Instant,
    ) {
        self.offset = match banding {
            true => vec2(
                rubber_band(distance.x, dimensions.x),
                rubber_band(distance.y, dimensions.y),
            ),
            false => Vec2::ZERO,
        };
        self.samples.push_back((now, position + self.offset));
        while self.samples.len() > 2
            && self
                .samples
                .front()
                .is_some_and(|(when, _)| now.duration_since(*when).as_secs_f32() > VELOCITY_WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub fn release(&mut self, now: Instant) {
        self.held = false;
        self.stepped = now;
        self.velocity = match self.offset == Vec2::ZERO {
            true => Vec2::ZERO,
            false => self.measured_velocity(now),
        };
        self.samples.clear();
    }

    fn measured_velocity(&self, now: Instant) -> Vec2 {
        let (Some((first, from)), Some((last, to))) = (self.samples.front(), self.samples.back())
        else {
            return Vec2::ZERO;
        };
        if now.duration_since(*last).as_secs_f32() > VELOCITY_WINDOW {
            return Vec2::ZERO;
        }
        let elapsed = last.duration_since(*first).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return Vec2::ZERO;
        }
        (*to - *from) * elapsed.recip()
    }

    pub fn settle(&mut self) {
        self.offset = Vec2::ZERO;
        self.velocity = Vec2::ZERO;
    }

    pub fn moving(&self) -> bool {
        !self.held && (self.offset != Vec2::ZERO || self.velocity != Vec2::ZERO)
    }

    pub fn step(
        &mut self,
        now: Instant,
        position: &mut Pos2,
        limit: impl Fn(Pos2) -> Pos2,
        banding: bool,
    ) -> bool {
        if !self.moving() {
            return false;
        }
        let elapsed = now.duration_since(self.stepped).as_secs_f32();
        self.stepped = now;
        let mut raw = *position;
        for (offset, velocity, raw) in [
            (&mut self.offset.x, &mut self.velocity.x, &mut raw.x),
            (&mut self.offset.y, &mut self.velocity.y, &mut raw.y),
        ] {
            if *offset != 0.0 {
                spring_back(offset, velocity, elapsed, self.spring);
            } else if *velocity != 0.0 {
                let decay = (-GLIDE_FRICTION * elapsed).exp();
                *raw += *velocity * (1.0 - decay) / GLIDE_FRICTION;
                *velocity *= decay;
                if velocity.abs() < MINIMUM_VELOCITY {
                    *velocity = 0.0;
                }
            }
        }
        *position = limit(raw);
        for (offset, velocity, past) in [
            (&mut self.offset.x, &mut self.velocity.x, raw.x - position.x),
            (&mut self.offset.y, &mut self.velocity.y, raw.y - position.y),
        ] {
            if past == 0.0 {
                continue;
            }
            match banding {
                true => *offset = past,
                false => *velocity = 0.0,
            }
        }
        true
    }
}
