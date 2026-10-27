use beui::{Vec2, native::Draw, native::DrawAt};

use super::*;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn at(rect: [f32; 4], clip: [f32; 4]) -> DrawAt {
    DrawAt {
        rect,
        clip,
        screen: Vec2::new(8.0, 8.0),
        pixels_per_point: 1.0,
        format: FORMAT,
        renderer: 1,
    }
}

mod a_clipped_texture_keeps_the_part_of_its_picture_inside_the_clip;
mod a_texture_is_drawn_into_its_rect_and_nowhere_else;
