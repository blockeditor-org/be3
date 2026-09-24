use super::*;

mod each_screen_shows_its_own_part_of_the_desktop;

use beui::{Color32, Context, RawInput, pos2};

use crate::test_client::{read, vulkan_device};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 24;

fn target(gpu: &Gpu) -> wgpu::Texture {
    gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("screen test target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * WIDTH + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}
