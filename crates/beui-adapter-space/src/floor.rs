use beui_core::color::Color32;
use glam::Vec3;

use crate::head::EYE_HEIGHT;
use crate::space::SKY;

pub const GROUND: Color32 = Color32::from_rgb(29, 31, 36);
pub const GRID: Color32 = Color32::from_rgb(89, 97, 108);
pub const AXIS: Color32 = Color32::from_rgb(111, 124, 149);
pub const EXTENT: f32 = 60.0;
pub const FADE_START: f32 = 8.0;
pub const FADE_END: f32 = 54.0;

pub fn fade(distance: f32) -> f32 {
    1.0 - smoothstep(FADE_START, FADE_END, distance)
}

pub fn ground_share(direction: Vec3) -> f32 {
    let down = -direction.y;
    if down <= 0.0 {
        return 0.0;
    }
    let along = (direction.x * direction.x + direction.z * direction.z).sqrt();
    fade(EYE_HEIGHT * along / down)
}

pub fn background(direction: Vec3) -> Color32 {
    let share = ground_share(direction);
    let sky = SKY.to_array();
    let ground = GROUND.to_array();
    let mix = |at: usize| {
        (f32::from(sky[at]) + (f32::from(ground[at]) - f32::from(sky[at])) * share).round() as u8
    };
    Color32::from_rgb(mix(0), mix(1), mix(2))
}

pub fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
