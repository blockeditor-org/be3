mod bundles;
mod looks;
mod queries;
mod settling;
mod tables;
pub mod transport;
mod webgpu;

#[cfg(test)]
mod tests;

use tables::{HashMap, HashSet};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use settling::Settling;
use tables::Table;

pub use settling::Wake;

pub use be_wasm_gpu_abi;
pub use wgpu;

const SURFACE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
    .union(wgpu::TextureUsages::TEXTURE_BINDING)
    .union(wgpu::TextureUsages::COPY_SRC);

static GENERATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub type Imports = Arc<Mutex<HashMap<u64, wgpu::Texture>>>;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    buffers: Table<wgpu::Buffer>,
    textures: Table<wgpu::Texture>,
    made_textures: HashSet<u32>,
    views: Table<wgpu::TextureView>,
    samplers: Table<wgpu::Sampler>,
    group_layouts: Table<wgpu::BindGroupLayout>,
    groups: Table<wgpu::BindGroup>,
    pipeline_layouts: Table<wgpu::PipelineLayout>,
    modules: Table<wgpu::ShaderModule>,
    pipelines: Table<wgpu::RenderPipeline>,
    settling: Settling,
    encoders: Table<wgpu::CommandEncoder>,
    command_buffers: Table<wgpu::CommandBuffer>,
    passes: Table<webgpu::Recording<wgpu::RenderPass<'static>>>,
    compute_pipelines: Table<wgpu::ComputePipeline>,
    compute_passes: Table<webgpu::Recording<wgpu::ComputePass<'static>>>,
    query_sets: Table<queries::QuerySet>,
    coarsening: Option<queries::Coarsening>,
    bundle_encoders: Table<bundles::BundleEncoder>,
    bundles: Table<wgpu::RenderBundle>,
    surfaces: HashMap<u32, Surface>,
    imports: Imports,
    presented: Vec<u32>,
    /// The texture each surface showed when the app last presented it, and its generation:
    /// what its host draws, whatever the app configured since.
    last_presented: HashMap<u32, (wgpu::Texture, u64)>,
    /// The surfaces let go of since `take_released`.
    released: Vec<u32>,
    error: Option<String>,
    /// The state of wasi:webgpu's objects beyond the tables above (webgpu.rs).
    web: webgpu::Web,
    looks: looks::Looks,
}

type Outcome<T> = Result<T, String>;

struct Surface {
    textures: Vec<(wgpu::Texture, u64)>,
    /// The formats its textures may also be viewed as.
    view_formats: Vec<wgpu::TextureFormat>,
    /// How its configuration asks it to be shown, and, unless as it is, the textures each of
    /// `textures` is drawn into at its present, which are what is shown (looks.rs).
    look: looks::Look,
    shown: Vec<(wgpu::Texture, u64)>,
    drawn: usize,
    ages: Ages,
}

pub(crate) struct Ages {
    presented: Vec<Option<u64>>,
    frames: u64,
    drawn: usize,
}

impl Ages {
    pub(crate) fn new(textures: usize) -> Self {
        Self {
            presented: vec![None; textures],
            frames: 0,
            drawn: 0,
        }
    }

    pub(crate) fn age(&self) -> u32 {
        match self.presented[self.drawn] {
            Some(frame) => (self.frames + 1 - frame) as u32,
            None => 0,
        }
    }

    pub(crate) fn present(&mut self) {
        self.frames += 1;
        self.presented[self.drawn] = Some(self.frames);
        self.drawn = (self.drawn + 1) % self.presented.len();
    }
}

impl Surface {
    /// The texture last presented, as it is shown.
    fn shown(&self) -> &(wgpu::Texture, u64) {
        let last = (self.drawn + 1) % self.textures.len();
        self.shown.get(last).unwrap_or(&self.textures[last])
    }
}

/// Whether a gpu import only records into a render pass, compute pass or render bundle (set the
/// pipeline, a viewport, a buffer, draw, dispatch, replay bundles with `execute-bundles`): a few
/// nanoseconds of appending to a list, made thousands of times a frame. A host times every other
/// gpu import for its "GPU calls" total and leaves these untimed, since reading the clock around
/// each cost far more than the call (about 60 ns of 105 a call in be-wasm-host's
/// example-gpu-calls); their time counts in the step's. Ending a pass and finishing a bundle are
/// timed: that is where wgpu validates and encodes everything recorded.
pub fn only_records(function: &str) -> bool {
    [
        "[method]gpu-render-pass-encoder.",
        "[method]gpu-compute-pass-encoder.",
        "[method]gpu-render-bundle-encoder.",
    ]
    .iter()
    .find_map(|pass| function.strip_prefix(pass))
    .is_some_and(|method| !matches!(method, "end" | "finish"))
}

/// Whether a gpu import submits work to the queue, which a host times apart.
pub fn submits(function: &str) -> bool {
    function == "[method]gpu-queue.submit"
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            buffers: Table::new(),
            textures: Table::new(),
            made_textures: HashSet::default(),
            views: Table::new(),
            samplers: Table::new(),
            group_layouts: Table::new(),
            groups: Table::new(),
            pipeline_layouts: Table::new(),
            modules: Table::new(),
            pipelines: Table::new(),
            settling: Settling::new(),
            encoders: Table::new(),
            command_buffers: Table::new(),
            passes: Table::new(),
            compute_pipelines: Table::new(),
            compute_passes: Table::new(),
            query_sets: Table::new(),
            coarsening: None,
            bundle_encoders: Table::new(),
            bundles: Table::new(),
            surfaces: HashMap::default(),
            imports: Imports::default(),
            presented: Vec::new(),
            last_presented: HashMap::default(),
            released: Vec::new(),
            error: None,
            web: webgpu::Web::default(),
            looks: looks::Looks::default(),
        }
    }

    pub fn sharing(mut self, imports: Imports) -> Self {
        self.imports = imports;
        self
    }

    pub fn waking(mut self, wake: Wake) -> Self {
        self.settling.waking(wake);
        self
    }

    pub fn owning_device(self) -> Self {
        self.device.set_device_lost_callback(self.settling.losing());
        self
    }

    pub fn imports(&self) -> Imports {
        Arc::clone(&self.imports)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn attach_surface(&mut self, surface: u32, texture: wgpu::Texture) {
        self.insert_surface(surface, vec![texture], Vec::new());
    }

    fn insert_surface(
        &mut self,
        surface: u32,
        textures: Vec<wgpu::Texture>,
        view_formats: Vec<wgpu::TextureFormat>,
    ) {
        let textures: Vec<(wgpu::Texture, u64)> = textures
            .into_iter()
            .map(|texture| {
                let generation = GENERATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                (texture, generation)
            })
            .collect();
        let ages = Ages::new(textures.len());
        self.surfaces.insert(
            surface,
            Surface {
                textures,
                view_formats,
                look: looks::Look::default(),
                shown: Vec::new(),
                drawn: 0,
                ages,
            },
        );
    }

    /// Shows `surface` as `look` asks: as it is, or drawn at each present into textures of
    /// its own, made here when its look or its textures changed.
    fn look(&mut self, surface: u32, look: looks::Look) {
        let Some(target) = self.surfaces.get_mut(&surface) else {
            return;
        };
        let kept = target.look == look && (look.plain() || target.shown.len() == target.textures.len());
        if kept {
            return;
        }
        target.look = look;
        target.shown = match look.plain() {
            true => Vec::new(),
            false => target
                .textures
                .iter()
                .map(|(canvas, _)| {
                    let shown = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("app canvas, shown"),
                        size: canvas.size(),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: canvas.format(),
                        usage: SURFACE_USAGE,
                        view_formats: &[],
                    });
                    let generation = GENERATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    (shown, generation)
                })
                .collect(),
        };
    }

    /// Draws the texture `surface` just presented into what shows it, when its look asks.
    fn show(&mut self, surface: u32) {
        let Some(target) = self.surfaces.get(&surface) else {
            return;
        };
        let last = (target.drawn + 1) % target.textures.len();
        let (Some((shown, _)), (canvas, _)) = (target.shown.get(last), &target.textures[last]) else {
            return;
        };
        self.looks.show(&self.device, &self.queue, target.look, canvas, shown);
    }

    pub fn detach_surface(&mut self, surface: u32) {
        if self.surfaces.remove(&surface).is_some() {
            self.presented.retain(|presented| *presented != surface);
            self.last_presented.remove(&surface);
            self.released.push(surface);
        }
    }

    /// The surfaces let go of since the last call, by the app or the host, each once.
    pub fn take_released(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.released)
    }

    pub fn surface(&self, surface: u32) -> Option<(&wgpu::Texture, u64)> {
        let (texture, generation) = self.surfaces.get(&surface)?.shown();
        Some((texture, *generation))
    }

    /// The texture `surface` showed when it was last presented, and its generation: a surface
    /// configured afresh since (a new size) still shows it until it is presented again.
    pub fn presented_surface(&self, surface: u32) -> Option<(&wgpu::Texture, u64)> {
        let (texture, generation) = self.last_presented.get(&surface)?;
        Some((texture, *generation))
    }

    pub fn take_presented(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.presented)
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    pub fn report(&mut self, message: String) {
        if self.error.is_none() && !self.lost() {
            self.error = Some(message);
        }
    }

    fn lost(&self) -> bool {
        self.settling.is_lost()
    }
}

fn slice(buffer: &wgpu::Buffer, offset: u64, size: Option<u64>) -> Outcome<wgpu::BufferSlice<'_>> {
    let end = match size {
        None => Some(buffer.size()),
        Some(size) => offset.checked_add(size),
    };
    match end {
        Some(end) if offset < end && end <= buffer.size() => Ok(buffer.slice(offset..end)),
        _ => Err(format!(
            "a binding of {offset}..{} is empty or runs past the end of its {}-byte buffer",
            end.map_or_else(|| "overflow".to_owned(), |end| end.to_string()),
            buffer.size()
        )),
    }
}

/// The slice a vertex buffer slot is set to, or `None` to unbind it, which takes no offset or
/// size, as WebGPU's `setVertexBuffer(slot, null)` and wgpu-core say.
fn vertex_slice(
    buffer: Option<&wgpu::Buffer>,
    slot: u32,
    offset: u64,
    size: Option<u64>,
) -> Outcome<Option<wgpu::BufferSlice<'_>>> {
    match (buffer, size) {
        (Some(buffer), _) => slice(buffer, offset, size).map(Some),
        (None, _) if offset != 0 => Err(format!(
            "unbinding the vertex buffer at slot {slot} takes an offset of 0, not {offset}"
        )),
        (None, Some(size)) if size != 0 => Err(format!(
            "unbinding the vertex buffer at slot {slot} takes no size, not {size}"
        )),
        (None, _) => Ok(None),
    }
}

pub(crate) fn caught(popped: impl Future<Output = Option<wgpu::Error>>) -> Option<String> {
    match pin!(popped).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(error) => error.map(|error| error.to_string()),
        Poll::Pending => None,
    }
}
