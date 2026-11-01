mod bundles;
mod convert;
mod host;
mod queries;
mod settling;
mod tables;
pub mod transport;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::{Pin, pin};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use block_gpu_abi::device as abi;
use settling::Settling;
use tables::Table;

pub use settling::Wake;

pub use block_gpu_abi;
pub use wgpu;

const SURFACE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
    .union(wgpu::TextureUsages::TEXTURE_BINDING)
    .union(wgpu::TextureUsages::COPY_SRC);

pub type Imports = Arc<Mutex<HashMap<u64, wgpu::Texture>>>;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    limits: abi::SupportedLimits,
    buffers: Table<wgpu::Buffer>,
    textures: Table<wgpu::Texture>,
    made_textures: HashSet<u32>,
    views: Table<wgpu::TextureView>,
    samplers: Table<wgpu::Sampler>,
    group_layouts: Table<wgpu::BindGroupLayout>,
    groups: Table<wgpu::BindGroup>,
    pipeline_layouts: Table<wgpu::PipelineLayout>,
    modules: Table<wgpu::ShaderModule>,
    compilations: HashMap<u32, Compilation>,
    pipelines: Table<wgpu::RenderPipeline>,
    settling: Settling,
    destroyed: bool,
    encoders: Table<wgpu::CommandEncoder>,
    command_buffers: Table<wgpu::CommandBuffer>,
    passes: Table<wgpu::RenderPass<'static>>,
    compute_pipelines: Table<wgpu::ComputePipeline>,
    compute_passes: Table<wgpu::ComputePass<'static>>,
    query_sets: Table<queries::QuerySet>,
    coarsening: Option<queries::Coarsening>,
    bundle_encoders: Table<bundles::BundleEncoder>,
    bundles: Table<wgpu::RenderBundle>,
    surface_ids: Table<u32>,
    surfaces: HashMap<u32, Surface>,
    imports: Imports,
    presented: Vec<u32>,
    generation: u64,
    error: Option<String>,
}

type Outcome<T> = Result<T, String>;

#[cfg(not(target_arch = "wasm32"))]
type Compilation = Pin<Box<dyn Future<Output = wgpu::CompilationInfo> + Send>>;
#[cfg(target_arch = "wasm32")]
type Compilation = Pin<Box<dyn Future<Output = wgpu::CompilationInfo>>>;

struct Surface {
    textures: Vec<(wgpu::Texture, u64)>,
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
    fn shown(&self) -> &(wgpu::Texture, u64) {
        &self.textures[(self.drawn + 1) % self.textures.len()]
    }
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let limits = convert::limits(&device.limits());
        Self {
            device,
            queue,
            limits,
            buffers: Table::new(),
            textures: Table::new(),
            made_textures: HashSet::new(),
            views: Table::new(),
            samplers: Table::new(),
            group_layouts: Table::new(),
            groups: Table::new(),
            pipeline_layouts: Table::new(),
            modules: Table::new(),
            compilations: HashMap::new(),
            pipelines: Table::new(),
            settling: Settling::new(),
            destroyed: false,
            encoders: Table::new(),
            command_buffers: Table::new(),
            passes: Table::new(),
            compute_pipelines: Table::new(),
            compute_passes: Table::new(),
            query_sets: Table::new(),
            coarsening: None,
            bundle_encoders: Table::new(),
            bundles: Table::new(),
            surface_ids: Table::new(),
            surfaces: HashMap::new(),
            imports: Imports::default(),
            presented: Vec::new(),
            generation: 0,
            error: None,
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
        self.insert_surface(surface, vec![texture]);
    }

    fn insert_surface(&mut self, surface: u32, textures: Vec<wgpu::Texture>) {
        let textures: Vec<(wgpu::Texture, u64)> = textures
            .into_iter()
            .map(|texture| {
                self.generation += 1;
                (texture, self.generation)
            })
            .collect();
        let ages = Ages::new(textures.len());
        self.surfaces.insert(
            surface,
            Surface {
                textures,
                drawn: 0,
                ages,
            },
        );
    }

    pub fn detach_surface(&mut self, surface: u32) {
        self.surfaces.remove(&surface);
    }

    pub fn surface(&self, surface: u32) -> Option<(&wgpu::Texture, u64)> {
        let (texture, generation) = self.surfaces.get(&surface)?.shown();
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

    fn fail<T>(&mut self, result: Outcome<T>, fallback: T) -> T {
        match result {
            Ok(value) => value,
            Err(message) => {
                self.report(message);
                fallback
            }
        }
    }

    fn lost(&self) -> bool {
        self.destroyed || self.settling.lost().is_some()
    }

    fn create<T>(
        &mut self,
        table: fn(&mut Self) -> &mut Table<T>,
        act: impl FnOnce(&mut Self) -> Outcome<T>,
    ) -> Option<u32> {
        if self.destroyed {
            return Some(table(self).skip());
        }
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let created = act(self);
        let caught = caught(scope.pop());
        let result = match (created, caught) {
            (Ok(_), Some(error)) | (Err(error), _) => {
                table(self).skip();
                Err(error)
            }
            (Ok(value), None) => Ok(table(self).insert(value)),
        };
        self.fail(result.map(Some), None)
    }

    fn guard(&mut self, act: impl FnOnce(&mut Self) -> Outcome<()>) {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = act(self);
        let result = match caught(scope.pop()) {
            Some(error) => result.and(Err(error)),
            None => result,
        };
        self.fail(result, ());
    }

    fn surface_id(&mut self, surface: abi::Surface) -> Option<u32> {
        let found = self.surface_ids.get(surface.0, "surface").copied();
        self.fail(found.map(Some), None)
    }
}

fn texel_copy_texture(
    texture: &wgpu::Texture,
    info: abi::TexelCopyTextureInfo,
) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: info.mip_level,
        origin: wgpu::Origin3d {
            x: info.origin.x,
            y: info.origin.y,
            z: info.origin.z,
        },
        aspect: wgpu::TextureAspect::from(info.aspect),
    }
}

fn texel_copy_buffer(
    buffer: &wgpu::Buffer,
    layout: abi::TexelCopyBufferLayout,
) -> wgpu::TexelCopyBufferInfo<'_> {
    wgpu::TexelCopyBufferInfo {
        buffer,
        layout: wgpu::TexelCopyBufferLayout {
            offset: layout.offset,
            bytes_per_row: layout.bytes_per_row,
            rows_per_image: layout.rows_per_image,
        },
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

fn caught(popped: impl Future<Output = Option<wgpu::Error>>) -> Option<String> {
    match pin!(popped).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(error) => error.map(|error| error.to_string()),
        Poll::Pending => None,
    }
}
