use std::collections::BTreeMap;

use beui::{Color32, FrameOutput, Vec2};
use paint_snapshot::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};

#[cfg(target_arch = "wasm32")]
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
#[cfg(target_arch = "wasm32")]
const SURFACE: u32 = 0;
const SCREEN: paint_snapshot::TextureKey = 0;

pub(crate) fn capture(
    output: &FrameOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
) -> Result<Snapshot, String> {
    let width = (size.x * pixels_per_point).round().max(1.0) as u32;
    let height = (size.y * pixels_per_point).round().max(1.0) as u32;
    let pixels = render(output, [width, height], pixels_per_point, background)?;
    let texture = Texture::encode([width, height], &pixels)?;
    Ok(Snapshot::of(
        Frame {
            size: [width, height],
            pixels_per_point,
            background: background.to_array(),
            primitives: vec![Primitive {
                clip: [0.0, 0.0, size.x, size.y],
                content: Content::Mesh(quad(size)),
            }],
        },
        BTreeMap::from([(SCREEN, texture)]),
    ))
}

fn quad(size: Vec2) -> Vec<Triangle> {
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color: [255, 255, 255, 255],
    };
    let top_left = corner(0.0, 0.0, 0.0, 0.0);
    let top_right = corner(size.x, 0.0, 1.0, 0.0);
    let bottom_left = corner(0.0, size.y, 0.0, 1.0);
    let bottom_right = corner(size.x, size.y, 1.0, 1.0);
    vec![
        Triangle {
            texture: SCREEN,
            corners: [top_left, top_right, bottom_left],
        },
        Triangle {
            texture: SCREEN,
            corners: [top_right, bottom_right, bottom_left],
        },
    ]
}

#[cfg(not(target_arch = "wasm32"))]
fn render(
    _output: &FrameOutput,
    _size: [u32; 2],
    _pixels_per_point: f32,
    _background: Color32,
) -> Result<Vec<[u8; 4]>, String> {
    Err(
        "a plugin paints through the gpu abi, so its tests only run in wasm: scripts/internal/test-plugins.sh"
            .into(),
    )
}

#[cfg(target_arch = "wasm32")]
fn render(
    output: &FrameOutput,
    size: [u32; 2],
    pixels_per_point: f32,
    background: Color32,
) -> Result<Vec<[u8; 4]>, String> {
    use beui::{clear_color, Renderer};

    let [width, height] = size;
    let (device, queue) = block_gpu_guest::device_and_queue();
    block_gpu_guest::configure_surface(SURFACE, width, height, FORMAT);
    let target = block_gpu_guest::acquire_surface_texture(SURFACE)?;
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer = Renderer::new(&device, FORMAT);
    renderer.prepare(
        &device,
        &queue,
        output,
        beui::vec2(width as f32, height as f32),
        pixels_per_point,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("beui editor encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui editor pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color(background)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.paint(&mut pass);
    }
    queue.submit(Some(encoder.finish()));
    block_gpu_guest::present_surface(SURFACE);

    read(SURFACE, width, height)
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "be3_test")]
unsafe extern "C" {
    fn surface_read(surface: u32, pointer: u32, capacity: u32) -> u32;
}

#[cfg(target_arch = "wasm32")]
fn read(surface: u32, width: u32, height: u32) -> Result<Vec<[u8; 4]>, String> {
    let needed = (width as usize) * (height as usize) * 4;
    let mut bytes = vec![0u8; needed];
    let written =
        unsafe { surface_read(surface, bytes.as_mut_ptr() as u32, bytes.len() as u32) } as usize;
    if written != needed {
        return Err(format!(
            "the host read {written} bytes of a {width} by {height} painting, which needs {needed}"
        ));
    }
    Ok(bytes.as_chunks::<4>().0.to_vec())
}
