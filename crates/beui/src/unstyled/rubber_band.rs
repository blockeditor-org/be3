#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::time::Instant;

use crate::geometry::{Vec2, vec2};

const RUBBER_BAND_FACTOR: f32 = 0.55;
const SETTLED_DISTANCE: f32 = 0.25;
const VELOCITY_WINDOW: f32 = 0.08;
pub(crate) const MINIMUM_VELOCITY: f32 = 5.0;
pub(crate) const MAX_ANIMATION_STEP: f32 = 0.05;

#[derive(Clone, Copy)]
pub(crate) struct Spring {
    stiffness: f32,
    damping: f32,
}

pub(crate) const SCROLL_SPRING: Spring = Spring {
    stiffness: 180.0,
    damping: 24.0,
};

pub(crate) const WINDOW_SPRING: Spring = Spring {
    stiffness: 400.0,
    damping: 38.0,
};

pub(crate) fn rubber_band(distance: f32, dimension: f32) -> f32 {
    if distance == 0.0 {
        return 0.0;
    }
    let dimension = dimension.max(1.0);
    let magnitude =
        dimension * (1.0 - 1.0 / (distance.abs() * RUBBER_BAND_FACTOR / dimension + 1.0));
    magnitude.copysign(distance)
}

pub(crate) fn unband(overscroll: f32, dimension: f32) -> f32 {
    if overscroll == 0.0 {
        return 0.0;
    }
    let dimension = dimension.max(1.0);
    let stretch = (overscroll.abs() / dimension).min(0.99);
    (dimension * stretch / (RUBBER_BAND_FACTOR * (1.0 - stretch))).copysign(overscroll)
}

pub(crate) fn spring_back(overscroll: &mut f32, velocity: &mut f32, elapsed: f32, spring: Spring) {
    let acceleration = -spring.stiffness * *overscroll - spring.damping * *velocity;
    *velocity += acceleration * elapsed;
    *overscroll += *velocity * elapsed;
    if overscroll.abs() < SETTLED_DISTANCE && velocity.abs() < MINIMUM_VELOCITY {
        *overscroll = 0.0;
        *velocity = 0.0;
    }
}

pub(crate) struct Band {
    pub(crate) offset: Vec2,
    velocity: Vec2,
    held: bool,
    stepped: Instant,
    samples: VecDeque<(Instant, Vec2)>,
    spring: Spring,
}

impl Band {
    pub(crate) fn new(spring: Spring) -> Self {
        Self {
            offset: Vec2::ZERO,
            velocity: Vec2::ZERO,
            held: false,
            stepped: Instant::now(),
            samples: VecDeque::new(),
            spring,
        }
    }

    pub(crate) fn grab(&mut self, dimensions: Vec2) -> Vec2 {
        self.held = true;
        self.velocity = Vec2::ZERO;
        self.samples.clear();
        vec2(
            unband(self.offset.x, dimensions.x),
            unband(self.offset.y, dimensions.y),
        )
    }

    pub(crate) fn stretch(
        &mut self,
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
        self.samples.push_back((now, self.offset));
        while self.samples.len() > 2
            && self
                .samples
                .front()
                .is_some_and(|(when, _)| now.duration_since(*when).as_secs_f32() > VELOCITY_WINDOW)
        {
            self.samples.pop_front();
        }
    }

    pub(crate) fn release(&mut self, now: Instant) {
        self.held = false;
        self.stepped = now;
        self.velocity = self.measured_velocity(now);
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

    pub(crate) fn moving(&self) -> bool {
        !self.held && (self.offset != Vec2::ZERO || self.velocity != Vec2::ZERO)
    }

    pub(crate) fn step(&mut self, now: Instant) -> bool {
        if !self.moving() {
            return false;
        }
        let elapsed = now
            .duration_since(self.stepped)
            .as_secs_f32()
            .min(MAX_ANIMATION_STEP);
        self.stepped = now;
        spring_back(
            &mut self.offset.x,
            &mut self.velocity.x,
            elapsed,
            self.spring,
        );
        spring_back(
            &mut self.offset.y,
            &mut self.velocity.y,
            elapsed,
            self.spring,
        );
        true
    }
}
