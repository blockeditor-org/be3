//! Times beui's wgpu renderer drawing fixed scenes into an offscreen target (the suite's
//! `renderer`, guides/performance.md): a page of text, a canvas of 10,000 shapes panned and
//! zoomed, a blur swept across radii, nested opacity groups, and a small damaged rect inside a
//! full window. For each it reports the CPU time to prepare, encode and submit a frame, and the
//! GPU time the frame took from timestamp queries around it. The scenes are painted once per
//! frame before the clock starts, so they measure the renderer, not beui's painting.
//!
//! It names the adapter it ran on and reports nothing from a software one (lavapipe, llvmpipe),
//! whose times say nothing about a GPU; there it still draws a few frames of every scene, which
//! is how a machine without a GPU checks that it runs.

use std::time::{Duration, Instant};

use beui_core::context::Context;
use beui_core::document::Document;
use beui_core::filter::Filter;
use beui_core::node::SpaceId;
use beui_core::painter::{Painter, Zoom};
use beui_renderer_wgpu::{Renderer, Repaint, Resources, wgpu_color_in};
use beui_types::{Color32, FillRule, FontId, NodeId, Path, Rect, Stroke, pos2, vec2};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const BACKGROUND: Color32 = Color32::from_rgb(250, 250, 248);
const FRAMES: usize = 60;
const WARM_UP: usize = 2;
const SOFTWARE_FRAMES: usize = 3;
const BLUR_RADII: [f32; 8] = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];

fn main() {
    let gpu = Gpu::open();
    let info = gpu.adapter.get_info();
    let software = info.device_type == wgpu::DeviceType::Cpu
        || ["llvmpipe", "lavapipe", "swiftshader"]
            .iter()
            .any(|name| info.name.to_lowercase().contains(name));
    println!(
        "adapter: {} ({:?}, {:?}, driver {} {})",
        info.name, info.device_type, info.backend, info.driver, info.driver_info
    );
    if gpu.timestamps.is_none() {
        println!("the adapter has no timestamp queries: GPU times are not reported");
    }
    let frames = match software {
        true => SOFTWARE_FRAMES,
        false => FRAMES,
    };
    let mut document = Document::new();
    let host = document.create_canvas().id();
    let groups: Vec<NodeId> = (0..STACKS * DEPTH)
        .map(|_| document.create_canvas().id())
        .collect();
    let mut bench = Bench::new(gpu);
    let scenes: Vec<(&str, Vec<Timed>)> = vec![
        ("text", bench.run(frames, |_| Repaint::Everything, text_page)),
        ("canvas-pan", bench.run(frames, |_| Repaint::Everything, |painter, frame| {
            canvas(painter, host, frame, frames, false)
        })),
        ("canvas-zoom", bench.run(frames, |_| Repaint::Everything, |painter, frame| {
            canvas(painter, host, frame, frames, true)
        })),
        ("opacity", bench.run(frames, |_| Repaint::Everything, |painter, frame| {
            nested_opacity(painter, &groups, frame)
        })),
        ("damage", bench.damaged(frames)),
    ];
    let blurs: Vec<(f32, Vec<Timed>)> = BLUR_RADII
        .iter()
        .map(|radius| {
            let timed = bench.run(frames, |_| Repaint::Everything, |painter, _| {
                blurred(painter, *radius)
            });
            (*radius, timed)
        })
        .collect();

    if software {
        be_tool_bench::refuse(&format!(
            "{} is a software adapter, so the renderer benchmark reports no times; run it on a GPU",
            info.name
        ));
        return;
    }
    for (name, timed) in &scenes {
        report(name, timed);
    }
    for (radius, timed) in &blurs {
        report_medians(&format!("blur-{radius}"), timed);
    }
}

struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    timestamps: Option<Timestamps>,
}

impl Gpu {
    fn open() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
            compatible_surface: None,
        }))
        .expect("no graphics adapter is available");
        let timed = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("renderer bench device"),
            required_features: match timed {
                true => wgpu::Features::TIMESTAMP_QUERY,
                false => wgpu::Features::empty(),
            },
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("the adapter did not provide a device");
        let timestamps = timed.then(|| Timestamps::new(&device, &queue));
        Self {
            adapter,
            device,
            queue,
            timestamps,
        }
    }
}

/// Two timestamps around a frame's work, each written by an empty compute pass, which needs
/// only `TIMESTAMP_QUERY` rather than writes inside an encoder.
struct Timestamps {
    set: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    read: wgpu::Buffer,
    period: f32,
}

impl Timestamps {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("renderer bench timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let size = 2 * std::mem::size_of::<u64>() as u64;
        let resolved = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("renderer bench resolved timestamps"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("renderer bench read timestamps"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            set,
            resolved,
            read,
            period: queue.get_timestamp_period(),
        }
    }

    fn write(&self, encoder: &mut wgpu::CommandEncoder, index: u32) {
        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("renderer bench timestamp"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &self.set,
                beginning_of_pass_write_index: Some(index),
                end_of_pass_write_index: None,
            }),
        });
    }

    fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.set, 0..2, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(&self.resolved, 0, &self.read, 0, self.resolved.size());
    }

    fn read(&self, device: &wgpu::Device) -> Duration {
        self.read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device never finished the frame");
        let ticks: [u64; 2] = {
            let mapped = self
                .read
                .slice(..)
                .get_mapped_range()
                .expect("the timestamps are mapped");
            bytemuck::pod_read_unaligned(&mapped)
        };
        self.read.unmap();
        let nanoseconds = ticks[1].saturating_sub(ticks[0]) as f64 * f64::from(self.period);
        Duration::from_nanos(nanoseconds as u64)
    }
}

#[derive(Clone, Copy)]
struct Timed {
    cpu: Duration,
    gpu: Option<Duration>,
}

struct Bench {
    gpu: Gpu,
    context: Context,
    renderer: Renderer,
    view: wgpu::TextureView,
}

impl Bench {
    fn new(gpu: Gpu) -> Self {
        let renderer = Renderer::new(&Resources::new(&gpu.device, FORMAT));
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("renderer bench target"),
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
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let context = Context::new(beui_font_skrifa::SkrifaFonts::default());
        context.stop_clock();
        Self {
            gpu,
            context,
            renderer,
            view,
        }
    }

    /// Draws `frames` frames of `paint` after `WARM_UP` more, which fill the glyph atlas and the
    /// renderer's buffers, and times each.
    fn run(
        &mut self,
        frames: usize,
        repaint: impl Fn(usize) -> Repaint,
        paint: impl Fn(&Painter, usize),
    ) -> Vec<Timed> {
        (0..WARM_UP + frames)
            .map(|frame| {
                let step = frame.saturating_sub(WARM_UP);
                let repaint = match frame {
                    0 => Repaint::Everything,
                    _ => repaint(step),
                };
                self.frame(repaint, |painter| paint(painter, step))
            })
            .skip(WARM_UP)
            .collect()
    }

    /// A window of text and shapes, then a 64-point square repainted inside it each frame as
    /// a cursor or a ticking clock would be.
    fn damaged(&mut self, frames: usize) -> Vec<Timed> {
        let spot = |step: usize| {
            let x = 80.0 + (step % 24) as f32 * 72.0;
            Rect::from_min_size(pos2(x, 480.0), vec2(64.0, 64.0))
        };
        self.run(
            frames,
            |step| Repaint::Region {
                region: spot(step).into(),
                background: BACKGROUND,
            },
            |painter, step| {
                text_page(painter, 0);
                let spot = spot(step);
                painter.rect_filled(spot.shrink(8.0), 6.0, Color32::from_rgb(40, 90, 200));
            },
        )
    }

    fn frame(&mut self, repaint: Repaint, paint: impl FnOnce(&Painter)) -> Timed {
        let output = self.context.run(|context| paint(&context.painter()));
        let Gpu {
            device,
            queue,
            timestamps,
            ..
        } = &self.gpu;
        let started = Instant::now();
        let effective = self.renderer.prepare(
            device,
            queue,
            output.main(),
            vec2(WIDTH as f32, HEIGHT as f32),
            1.0,
            repaint,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("renderer bench encoder"),
        });
        if let Some(timestamps) = timestamps {
            timestamps.write(&mut encoder, 0);
        }
        let load = match effective {
            Repaint::Region { .. } => wgpu::LoadOp::Load,
            Repaint::Everything => wgpu::LoadOp::Clear(wgpu_color_in(FORMAT, BACKGROUND)),
        };
        self.renderer.render(
            device,
            queue,
            &mut encoder,
            &self.view,
            (WIDTH, HEIGHT),
            load,
        );
        if let Some(timestamps) = timestamps {
            timestamps.write(&mut encoder, 1);
            timestamps.resolve(&mut encoder);
        }
        queue.submit(Some(encoder.finish()));
        let cpu = started.elapsed();
        let gpu = match timestamps {
            Some(timestamps) => Some(timestamps.read(device)),
            None => {
                device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .expect("the device never finished the frame");
                None
            }
        };
        Timed { cpu, gpu }
    }
}

fn report(name: &str, timed: &[Timed]) {
    let cpu: Vec<Duration> = timed.iter().map(|timed| timed.cpu).collect();
    be_tool_bench::time(&format!("{name}.cpu-median"), median(&cpu));
    be_tool_bench::time(&format!("{name}.cpu-worst"), worst(&cpu));
    let gpu: Vec<Duration> = timed.iter().filter_map(|timed| timed.gpu).collect();
    if !gpu.is_empty() {
        be_tool_bench::time(&format!("{name}.gpu-median"), median(&gpu));
        be_tool_bench::time(&format!("{name}.gpu-worst"), worst(&gpu));
    }
}

fn report_medians(name: &str, timed: &[Timed]) {
    let cpu: Vec<Duration> = timed.iter().map(|timed| timed.cpu).collect();
    be_tool_bench::time(&format!("{name}.cpu-median"), median(&cpu));
    let gpu: Vec<Duration> = timed.iter().filter_map(|timed| timed.gpu).collect();
    if !gpu.is_empty() {
        be_tool_bench::time(&format!("{name}.gpu-median"), median(&gpu));
    }
}

fn median(values: &[Duration]) -> Duration {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.get(sorted.len() / 2).copied().unwrap_or_default()
}

fn worst(values: &[Duration]) -> Duration {
    values.iter().copied().max().unwrap_or_default()
}

const WORDS: &str = "the retained display tree decides what to draw and damage by identity decides \
                     where so an unchanged node is not encoded again while a scroll copies what it \
                     already drew and the glyph atlas keeps every run it rasterised";

/// A page of text: a heading, then columns of body text at a few sizes and weights of grey.
fn text_page(painter: &Painter, _frame: usize) {
    let ink = Color32::from_rgb(30, 30, 34);
    painter.text(
        pos2(48.0, 32.0),
        "Renderer benchmark: a page of text",
        FontId::proportional(32.0),
        ink,
    );
    let words: Vec<&str> = WORDS.split(' ').collect();
    for column in 0..3 {
        for line in 0..56 {
            let start = (column * 7 + line * 3) % words.len();
            let text: Vec<&str> = words.iter().cycle().skip(start).take(11).copied().collect();
            let size = match line % 14 {
                0 => 18.0,
                _ => 13.0,
            };
            let grey = 30 + (line % 5) as u8 * 20;
            painter.text(
                pos2(48.0 + column as f32 * 620.0, 96.0 + line as f32 * 17.5),
                text.join(" "),
                FontId::proportional(size),
                Color32::from_rgb(grey, grey, grey + 4),
            );
        }
    }
}

/// 10,000 shapes in a 100 by 100 grid, through a camera that pans or zooms each frame as the
/// infinite canvas's does: rounded fills, outlines, lines, filled triangles and stroked curves.
fn canvas(painter: &Painter, host: NodeId, frame: usize, frames: usize, zoom: bool) {
    let progress = frame as f32 / frames.max(1) as f32;
    let (offset, scale) = match zoom {
        false => (vec2(-progress * 2400.0, -progress * 1200.0), 0.5),
        true => (vec2(-200.0, -100.0), 1.0 - 0.9 * progress),
    };
    let camera = painter.zoomed(
        SpaceId::inside(host, 1),
        offset,
        Zoom {
            scale,
            raster: scale,
        },
        Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH as f32, HEIGHT as f32)),
    );
    let stroke = Stroke::new(2.0);
    for row in 0..100 {
        for column in 0..100 {
            let at = pos2(column as f32 * 48.0, row as f32 * 48.0);
            let rect = Rect::from_min_size(at, vec2(36.0, 36.0));
            let hue = ((row * 7 + column * 13) % 255) as u8;
            let color = Color32::from_rgb(hue, 255 - hue, 160);
            match (row + column) % 5 {
                0 => camera.rect_filled(rect, 6.0, color),
                1 => camera.rect_stroke(rect, 4.0, 2.0, color),
                2 => camera.line(rect.left_top(), rect.right_bottom(), 3.0, color),
                3 => camera.fill_path(
                    &Path::new()
                        .move_to(pos2(rect.center().x, rect.top()))
                        .line_to(rect.right_bottom())
                        .line_to(rect.left_bottom())
                        .close(),
                    FillRule::NonZero,
                    color,
                ),
                _ => camera.stroke_path(
                    &Path::new()
                        .move_to(rect.left_bottom())
                        .cubic_to(rect.left_top(), rect.right_bottom(), rect.right_top()),
                    &stroke,
                    color,
                ),
            }
        }
    }
}

/// Coloured squares across the window, blurred as a whole by `radius`.
fn blurred(painter: &Painter, radius: f32) {
    for row in 0..12 {
        for column in 0..20 {
            let at = pos2(column as f32 * 96.0 + 16.0, row as f32 * 90.0 + 16.0);
            let hue = ((row * 31 + column * 17) % 255) as u8;
            painter.rect_filled(
                Rect::from_min_size(at, vec2(64.0, 56.0)),
                8.0,
                Color32::from_rgb(hue, 120, 255 - hue),
            );
        }
    }
    painter.ctx().apply_filter(Filter {
        region: Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH as f32, HEIGHT as f32)),
        blur: radius,
        ..Filter::default()
    });
}

const STACKS: usize = 24;
const DEPTH: usize = 8;

/// Stacks of opacity groups `DEPTH` deep, each holding a rect and the next group, with the
/// opacities moving each frame. `groups` holds a node for each group's mask space, kept for
/// every frame so a group keeps its identity between frames as a retained frame's does.
fn nested_opacity(painter: &Painter, groups: &[NodeId], frame: usize) {
    for stack in 0..STACKS {
        let origin = pos2((stack % 8) as f32 * 236.0 + 16.0, (stack / 8) as f32 * 350.0 + 16.0);
        let mut group = painter.clone();
        for depth in 0..DEPTH {
            let inset = depth as f32 * 12.0;
            let rect = Rect::from_min_size(
                origin + vec2(inset, inset),
                vec2(220.0 - 2.0 * inset, 330.0 - 2.0 * inset),
            );
            let opacity = 0.6 + 0.4 * (((frame + depth + stack) % 10) as f32 / 10.0);
            group = group.faded(
                SpaceId::inside(groups[stack * DEPTH + depth], 1),
                rect,
                [0.0; 4],
                opacity,
            );
            let shade = (40 + depth * 25) as u8;
            group.rect_filled(rect, 10.0, Color32::from_rgb(shade, 80, 255 - shade));
        }
    }
}
