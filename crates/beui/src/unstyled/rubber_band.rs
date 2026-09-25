use std::time::Instant;

use crate::geometry::{Vec2, vec2};

const RUBBER_BAND_FACTOR: f32 = 0.55;
const SPRING_DAMPING: f32 = 24.0;
const SPRING_STIFFNESS: f32 = 180.0;
const SETTLED_DISTANCE: f32 = 0.25;
pub(crate) const MINIMUM_VELOCITY: f32 = 5.0;
pub(crate) const MAX_ANIMATION_STEP: f32 = 0.05;

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

pub(crate) fn spring_back(overscroll: &mut f32, velocity: &mut f32, elapsed: f32) {
    let acceleration = -SPRING_STIFFNESS * *overscroll - SPRING_DAMPING * *velocity;
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
}

impl Band {
    pub(crate) fn new() -> Self {
        Self {
            offset: Vec2::ZERO,
            velocity: Vec2::ZERO,
            held: false,
            stepped: Instant::now(),
        }
    }

    pub(crate) fn grab(&mut self, dimensions: Vec2) -> Vec2 {
        self.held = true;
        self.velocity = Vec2::ZERO;
        vec2(
            unband(self.offset.x, dimensions.x),
            unband(self.offset.y, dimensions.y),
        )
    }

    pub(crate) fn stretch(&mut self, distance: Vec2, dimensions: Vec2, banding: bool) {
        self.offset = match banding {
            true => vec2(
                rubber_band(distance.x, dimensions.x),
                rubber_band(distance.y, dimensions.y),
            ),
            false => Vec2::ZERO,
        };
    }

    pub(crate) fn release(&mut self) {
        self.held = false;
        self.stepped = Instant::now();
    }

    pub(crate) fn moving(&self) -> bool {
        !self.held && self.offset != Vec2::ZERO
    }

    pub(crate) fn step(&mut self) -> bool {
        if !self.moving() {
            return false;
        }
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.stepped)
            .as_secs_f32()
            .min(MAX_ANIMATION_STEP);
        self.stepped = now;
        spring_back(&mut self.offset.x, &mut self.velocity.x, elapsed);
        spring_back(&mut self.offset.y, &mut self.velocity.y, elapsed);
        true
    }
}
