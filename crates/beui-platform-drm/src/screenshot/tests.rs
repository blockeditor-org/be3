use super::*;

mod a_display_at_a_smaller_scale_is_enlarged_to_the_largest;
mod displays_side_by_side_make_one_picture;

use beui::{pos2, vec2};

fn filled(width: u32, height: u32, color: [u8; 4]) -> Capture {
    Capture {
        width,
        height,
        rgba: color.repeat((width * height) as usize),
    }
}

fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width() + x) * 4) as usize;
    image.pixels()[at..at + 4].try_into().unwrap()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
