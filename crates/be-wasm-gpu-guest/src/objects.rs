use core::{
    fmt,
    future::Future,
    ops::Range,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use std::{cell::RefCell, collections::HashMap, collections::HashSet, sync::Arc};

use be_wasm_gpu_abi::guest::webgpu as gpu;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_ops::local::{Items, Ticket};
use wgpu::custom::*;

use crate::convert::{self, custom, to_web};
use crate::passes::{CommandEncoder, RenderBundleEncoder};

/// What waits for the host to settle it: each is answered by `settle`, from the tickets
/// wasi:webgpu's calls answer, so the futures wgpu is handed hold only numbers.
#[derive(Default)]
struct Waiting {
    maps: Vec<(Ticket<Result<(), web::MapAsyncError>>, web::GpuBuffer, BufferMapCallback)>,
    work: Vec<(Ticket<()>, BoxSubmittedWorkDoneCallback)>,
    pops: HashMap<u32, Ticket<Result<Option<web::GpuError>, web::PopErrorScopeError>>>,
    popped: HashMap<u32, Option<wgpu::Error>>,
    compiling: HashMap<u32, Ticket<web::GpuCompilationInfo>>,
    compiled: HashMap<u32, wgpu::CompilationInfo>,
    wakers: Vec<Waker>,
    lost: Option<(Ticket<web::GpuDeviceLostInfo>, BoxDeviceLostCallback)>,
    uncaptured: Option<(Items<web::GpuError>, Arc<dyn wgpu::UncapturedErrorHandler>)>,
    handler: Option<Arc<dyn wgpu::UncapturedErrorHandler>>,
    /// The app's open error scopes, as the host has them, each with an error the guest raised
    /// itself (where WebGPU throws and wgpu reports a validation error instead).
    scopes: Vec<(wgpu::ErrorFilter, Option<wgpu::Error>)>,
    next: u32,
}

thread_local! {
    static WAITING: RefCell<Waiting> = RefCell::new(Waiting::default());
    /// Buffers mapped at creation, whose mapped range starts zeroed rather than read back.
    static FRESH: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
}

fn waiting<R>(act: impl FnOnce(&mut Waiting) -> R) -> R {
    WAITING.with(|waiting| act(&mut waiting.borrow_mut()))
}

/// A pipeline the host compiled asynchronously, which the next `create_*_pipeline` takes in
/// place of compiling it again (`crate::create_render_pipeline_async`).
#[derive(Default)]
struct Prepared {
    render: Option<gpu::GpuRenderPipeline>,
    compute: Option<gpu::GpuComputePipeline>,
}

thread_local! {
    static PREPARED: RefCell<Prepared> = RefCell::new(Prepared::default());
}

/// A pipeline descriptor in wasi:webgpu's terms, with the constants it names, which must live
/// until the pipeline is made.
struct Described<D> {
    descriptor: D,
    _constants: [Option<gpu::RecordGpuPipelineConstantValue>; 2],
}

fn render_descriptor(desc: &wgpu::RenderPipelineDescriptor<'_>) -> Described<web::GpuRenderPipelineDescriptor> {
    let vertex_constants = convert::constants(&desc.vertex.compilation_options);
    let fragment_constants = desc
        .fragment
        .as_ref()
        .and_then(|fragment| convert::constants(&fragment.compilation_options));
    let descriptor = web::GpuRenderPipelineDescriptor {
        vertex: web::GpuVertexState {
            buffers: Some(
                desc.vertex
                    .buffers
                    .iter()
                    .map(|buffer| {
                        buffer.as_ref().map(|buffer| web::GpuVertexBufferLayout {
                            array_stride: buffer.array_stride,
                            step_mode: Some(to_web(buffer.step_mode)),
                            attributes: buffer
                                .attributes
                                .iter()
                                .map(|attribute| web::GpuVertexAttribute {
                                    format: to_web(attribute.format),
                                    offset: attribute.offset,
                                    shader_location: attribute.shader_location,
                                })
                                .collect(),
                        })
                    })
                    .collect(),
            ),
            module: convert::module_handle(desc.vertex.module),
            entry_point: desc.vertex.entry_point.map(str::to_owned),
            constants: vertex_constants.as_ref().map(|record| record.handle()),
        },
        primitive: Some(web::GpuPrimitiveState {
            topology: Some(to_web(desc.primitive.topology)),
            strip_index_format: desc.primitive.strip_index_format.map(to_web),
            front_face: Some(to_web(desc.primitive.front_face)),
            cull_mode: Some(match desc.primitive.cull_mode {
                None => web::GpuCullMode::None,
                Some(wgpu::Face::Front) => web::GpuCullMode::Front,
                Some(wgpu::Face::Back) => web::GpuCullMode::Back,
            }),
            unclipped_depth: Some(desc.primitive.unclipped_depth),
        }),
        depth_stencil: desc.depth_stencil.as_ref().map(|state| {
            let face = |face: wgpu::StencilFaceState| web::GpuStencilFaceState {
                compare: Some(to_web(face.compare)),
                fail_op: Some(to_web(face.fail_op)),
                depth_fail_op: Some(to_web(face.depth_fail_op)),
                pass_op: Some(to_web(face.pass_op)),
            };
            web::GpuDepthStencilState {
                format: to_web(state.format),
                depth_write_enabled: state.depth_write_enabled,
                depth_compare: state.depth_compare.map(to_web),
                stencil_front: Some(face(state.stencil.front)),
                stencil_back: Some(face(state.stencil.back)),
                stencil_read_mask: Some(state.stencil.read_mask),
                stencil_write_mask: Some(state.stencil.write_mask),
                depth_bias: Some(state.bias.constant),
                depth_bias_slope_scale: Some(state.bias.slope_scale),
                depth_bias_clamp: Some(state.bias.clamp),
            }
        }),
        multisample: Some(web::GpuMultisampleState {
            count: Some(desc.multisample.count),
            mask: Some(desc.multisample.mask as u32),
            alpha_to_coverage_enabled: Some(desc.multisample.alpha_to_coverage_enabled),
        }),
        fragment: desc.fragment.as_ref().map(|fragment| web::GpuFragmentState {
            targets: fragment
                .targets
                .iter()
                .map(|target| {
                    target.as_ref().map(|target| {
                        let component = |component: wgpu::BlendComponent| web::GpuBlendComponent {
                            operation: Some(to_web(component.operation)),
                            src_factor: Some(to_web(component.src_factor)),
                            dst_factor: Some(to_web(component.dst_factor)),
                        };
                        web::GpuColorTargetState {
                            format: to_web(target.format),
                            blend: target.blend.map(|blend| web::GpuBlendState {
                                color: component(blend.color),
                                alpha: component(blend.alpha),
                            }),
                            write_mask: Some(web::GpuColorWrite::from_bits(target.write_mask.bits())),
                        }
                    })
                })
                .collect(),
            module: convert::module_handle(fragment.module),
            entry_point: fragment.entry_point.map(str::to_owned),
            constants: fragment_constants.as_ref().map(|record| record.handle()),
        }),
        layout: convert::layout(desc.layout),
        label: convert::label(desc.label),
    };
    Described {
        descriptor,
        _constants: [vertex_constants, fragment_constants],
    }
}

fn compute_descriptor(desc: &wgpu::ComputePipelineDescriptor<'_>) -> Described<web::GpuComputePipelineDescriptor> {
    let constants = convert::constants(&desc.compilation_options);
    let descriptor = web::GpuComputePipelineDescriptor {
        compute: web::GpuProgrammableStage {
            module: convert::module_handle(desc.module),
            entry_point: desc.entry_point.map(str::to_owned),
            constants: constants.as_ref().map(|record| record.handle()),
        },
        layout: convert::layout(desc.layout),
        label: convert::label(desc.label),
    };
    Described {
        descriptor,
        _constants: [constants, None],
    }
}

/// The app's device behind `device`, which this backend made.
fn device_of(device: &wgpu::Device) -> &gpu::GpuDevice {
    &custom::<Device>(device.as_custom()).device
}

/// Polls the host once when `ticket` is not yet in, as `Settled` does.
async fn answered<T>(mut ticket: Ticket<T>) -> T {
    if let Some(value) = ticket.take() {
        return value;
    }
    settle(false);
    ticket.await
}

pub(crate) async fn render_pipeline_async(
    device: &wgpu::Device,
    desc: &wgpu::RenderPipelineDescriptor<'_>,
) -> Result<(), String> {
    let described = render_descriptor(desc);
    let ticket = device_of(device).create_render_pipeline_async(&described.descriptor);
    let made = answered(ticket).await.map_err(|error| error.message)?;
    PREPARED.with(|prepared| prepared.borrow_mut().render = Some(gpu::GpuRenderPipeline::from_handle(made)));
    Ok(())
}

pub(crate) async fn compute_pipeline_async(
    device: &wgpu::Device,
    desc: &wgpu::ComputePipelineDescriptor<'_>,
) -> Result<(), String> {
    let described = compute_descriptor(desc);
    let ticket = device_of(device).create_compute_pipeline_async(&described.descriptor);
    let made = answered(ticket).await.map_err(|error| error.message)?;
    PREPARED.with(|prepared| prepared.borrow_mut().compute = Some(gpu::GpuComputePipeline::from_handle(made)));
    Ok(())
}

#[derive(Debug)]
struct Raised(String);

impl fmt::Display for Raised {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Raised {}

/// Raises an error the guest found itself, as wgpu does: into the innermost validation scope,
/// or to the uncaptured error handler, or, with none, as a panic.
pub(crate) fn raise(message: String) {
    let error = wgpu::Error::Validation {
        source: Box::new(Raised(message.clone())),
        description: message,
    };
    let unscoped = waiting(|waiting| {
        match waiting
            .scopes
            .iter_mut()
            .rev()
            .find(|(filter, _)| *filter == wgpu::ErrorFilter::Validation)
        {
            Some((_, caught)) => {
                caught.get_or_insert(error);
                None
            }
            None => Some((error, waiting.handler.clone())),
        }
    });
    match unscoped {
        None => {}
        Some((error, Some(handler))) => handler(error),
        Some((error, None)) => panic!("wasi:webgpu refused a request: {error}"),
    }
}

/// A wasi:webgpu error as wgpu's.
fn error(handle: web::GpuError) -> wgpu::Error {
    let error = gpu::GpuError::from_handle(handle);
    let message = error.message();
    match error.kind() {
        web::GpuErrorKind::OutOfMemoryError => wgpu::Error::OutOfMemory {
            source: Box::new(Raised(message)),
        },
        web::GpuErrorKind::InternalError => wgpu::Error::Internal {
            source: Box::new(Raised(message.clone())),
            description: message,
        },
        web::GpuErrorKind::ValidationError => wgpu::Error::Validation {
            source: Box::new(Raised(message.clone())),
            description: message,
        },
    }
}

/// Takes what settled and answers the callbacks and futures waiting on it, all after the
/// waiting list is let go of, since a callback may ask the gpu for more. Answers whether nothing
/// still waits on the gpu.
pub(crate) fn settle(wait: bool) -> bool {
    // An app that never reached the gpu (one whose surfaces are not rendered) never polls it.
    if gpu::waiting() {
        gpu::settle(wait);
    }
    let mut maps = Vec::new();
    let mut work = Vec::new();
    let mut lost = None;
    let mut uncaptured = Vec::new();
    let mut wakers = Vec::new();
    let pending = waiting(|waiting| {
        for (mut ticket, buffer, callback) in std::mem::take(&mut waiting.maps) {
            match ticket.take() {
                Some(mapped) => maps.push((mapped, callback)),
                None => waiting.maps.push((ticket, buffer, callback)),
            }
        }
        for (mut ticket, callback) in std::mem::take(&mut waiting.work) {
            match ticket.take() {
                Some(()) => work.push(callback),
                None => waiting.work.push((ticket, callback)),
            }
        }
        let pops: Vec<u32> = waiting.pops.keys().copied().collect();
        for id in pops {
            let settled = waiting.pops.get_mut(&id).and_then(Ticket::take);
            if let Some(popped) = settled {
                waiting.pops.remove(&id);
                let caught = popped.ok().flatten().map(error);
                waiting.popped.insert(id, caught);
            }
        }
        let compiling: Vec<u32> = waiting.compiling.keys().copied().collect();
        for id in compiling {
            let settled = waiting.compiling.get_mut(&id).and_then(Ticket::take);
            if let Some(info) = settled {
                waiting.compiling.remove(&id);
                waiting.compiled.insert(id, compilation_info(info));
            }
        }
        if let Some((ticket, _)) = &mut waiting.lost
            && let Some(info) = ticket.take()
        {
            lost = waiting.lost.take().map(|(_, callback)| (info, callback));
        }
        if let Some((items, handler)) = &mut waiting.uncaptured {
            while let Some(raised) = items.try_next() {
                uncaptured.push((raised, Arc::clone(handler)));
            }
        }
        wakers = std::mem::take(&mut waiting.wakers);
        !waiting.maps.is_empty() || !waiting.work.is_empty()
    });
    for (mapped, callback) in maps {
        callback(mapped.map_err(|_| wgpu::BufferAsyncError));
    }
    for callback in work {
        callback();
    }
    if let Some((info, callback)) = lost {
        let info = gpu::GpuDeviceLostInfo::from_handle(info);
        let reason = match info.reason() {
            web::GpuDeviceLostReason::Destroyed => wgpu::DeviceLostReason::Destroyed,
            web::GpuDeviceLostReason::Unknown => wgpu::DeviceLostReason::Unknown,
        };
        callback(reason, info.message());
    }
    for (raised, handler) in uncaptured {
        handler(error(raised));
    }
    for waker in wakers {
        waker.wake();
    }
    !pending
}

fn compilation_info(info: web::GpuCompilationInfo) -> wgpu::CompilationInfo {
    let info = gpu::GpuCompilationInfo::from_handle(info);
    let messages = info
        .messages()
        .into_iter()
        .map(|message| {
            let message = gpu::GpuCompilationMessage::from_handle(message);
            let length = message.length();
            wgpu::CompilationMessage {
                message: message.message(),
                message_type: match message.r#type() {
                    web::GpuCompilationMessageType::Error => wgpu::CompilationMessageType::Error,
                    web::GpuCompilationMessageType::Warning => wgpu::CompilationMessageType::Warning,
                    web::GpuCompilationMessageType::Info => wgpu::CompilationMessageType::Info,
                },
                location: (message.line_num() > 0).then(|| wgpu::SourceLocation {
                    line_number: message.line_num() as u32,
                    line_position: message.line_pos() as u32,
                    offset: message.offset() as u32,
                    length: length as u32,
                }),
            }
        })
        .collect();
    wgpu::CompilationInfo { messages }
}

fn abort_maps(buffer: web::GpuBuffer) {
    let aborted: Vec<BufferMapCallback> = waiting(|waiting| {
        let (aborted, kept) = std::mem::take(&mut waiting.maps)
            .into_iter()
            .partition(|(_, mapped, _)| *mapped == buffer);
        waiting.maps = kept;
        aborted.into_iter().map(|(_, _, callback)| callback).collect()
    });
    for callback in aborted {
        callback(Err(wgpu::BufferAsyncError));
    }
}

macro_rules! resource {
    ($name:ident, $owner:ident) => {
        #[derive(Debug)]
        pub(crate) struct $name(pub(crate) gpu::$owner);
    };
}

pub(crate) use resource;

resource!(Buffer, GpuBuffer);
resource!(Texture, GpuTexture);
resource!(TextureView, GpuTextureView);
resource!(Sampler, GpuSampler);
resource!(BindGroupLayout, GpuBindGroupLayout);
resource!(BindGroup, GpuBindGroup);
resource!(PipelineLayout, GpuPipelineLayout);
resource!(RenderPipeline, GpuRenderPipeline);
resource!(ComputePipeline, GpuComputePipeline);
resource!(CommandBuffer, GpuCommandBuffer);
resource!(QuerySet, GpuQuerySet);
resource!(RenderBundle, GpuRenderBundle);
resource!(ShaderModule, GpuShaderModule);

impl Drop for Buffer {
    fn drop(&mut self) {
        let handle = self.0.handle();
        FRESH.with(|fresh| fresh.borrow_mut().remove(&handle.0));
        abort_maps(handle);
    }
}

/// A future answered once `settle` has the value its id names.
struct Settled<F> {
    id: u32,
    take: F,
}

impl<T, F: Fn(&mut Waiting, u32) -> Option<T> + Unpin> Future for Settled<F> {
    type Output = T;

    /// Polls the host once when the value is not yet in: a native host settles what it was
    /// asked at once, so a pop or compilation info is ready on the first poll there, as wgpu's
    /// own backends' are.
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<T> {
        if let Some(value) = waiting(|waiting| (self.take)(waiting, self.id)) {
            return Poll::Ready(value);
        }
        settle(false);
        let found = waiting(|waiting| match (self.take)(waiting, self.id) {
            Some(value) => Some(value),
            None => {
                waiting.wakers.push(context.waker().clone());
                None
            }
        });
        match found {
            Some(value) => Poll::Ready(value),
            None => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StagingBuffer {
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct MappedRange {
    buffer: web::GpuBuffer,
    offset: u64,
    bytes: Vec<u8>,
    written: bool,
}

impl Drop for MappedRange {
    fn drop(&mut self) {
        if !self.written {
            return;
        }
        let buffer = core::mem::ManuallyDrop::new(gpu::GpuBuffer::from_handle(self.buffer));
        let length = self.bytes.len() as u64;
        // A range outside what is mapped was refused when it was got.
        let _ = buffer.get_mapped_range_set_with_copy(&self.bytes, Some(self.offset), Some(length));
    }
}

pub(crate) struct Device {
    device: gpu::GpuDevice,
    features: wgpu::Features,
    limits: wgpu::Limits,
}

impl fmt::Debug for Device {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Device").field("device", &self.device).finish()
    }
}

#[derive(Debug)]
pub(crate) struct Queue(pub(crate) gpu::GpuQueue);

impl Device {
    pub(crate) fn new(device: gpu::GpuDevice, features: wgpu::Features, limits: wgpu::Limits) -> Self {
        Self {
            device,
            features,
            limits,
        }
    }
}

// unsafe(ffi)
impl DeviceInterface for Device {
    fn features(&self) -> wgpu::Features {
        self.features
    }

    fn limits(&self) -> wgpu::Limits {
        self.limits.clone()
    }

    fn adapter_info(&self) -> wgpu::AdapterInfo {
        let info = self.device.adapter_info();
        wgpu::AdapterInfo {
            name: info.description(),
            vendor: 0,
            device: 0,
            device_type: match info.is_fallback_adapter() {
                true => wgpu::DeviceType::Cpu,
                false => wgpu::DeviceType::Other,
            },
            device_pci_bus_id: String::new(),
            driver: info.vendor(),
            driver_info: info.architecture(),
            backend: wgpu::Backend::Noop,
            subgroup_min_size: info.subgroup_min_size(),
            subgroup_max_size: info.subgroup_max_size(),
            transient_saves_memory: Some(false),
            limit_bucket: None,
        }
    }

    fn create_shader_module(
        &self,
        desc: wgpu::ShaderModuleDescriptor<'_>,
        _shader_bound_checks: wgpu::ShaderRuntimeChecks,
    ) -> DispatchShaderModule {
        let code = match desc.source {
            wgpu::ShaderSource::Wgsl(source) => source.into_owned(),
            _ => panic!("wasi:webgpu only takes WGSL shaders, as browser WebGPU does"),
        };
        DispatchShaderModule::custom(ShaderModule(self.device.create_shader_module(&web::GpuShaderModuleDescriptor {
                code,
                compilation_hints: None,
                label: convert::label(desc.label),
            })))
    }

    unsafe fn create_shader_module_passthrough(
        &self,
        _desc: &wgpu::ShaderModuleDescriptorPassthrough<'_>,
    ) -> DispatchShaderModule {
        unimplemented!(
            "passthrough shaders are refused: apps are untrusted, and wgpu cannot validate them"
        )
    }

    fn create_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> DispatchBindGroupLayout {
        let descriptor = web::GpuBindGroupLayoutDescriptor {
            entries: desc.entries.iter().map(convert::layout_entry).collect(),
            label: convert::label(desc.label),
        };
        DispatchBindGroupLayout::custom(BindGroupLayout(self.device.create_bind_group_layout(&descriptor)))
    }

    fn create_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> DispatchBindGroup {
        let descriptor = web::GpuBindGroupDescriptor {
            layout: convert::group_layout_handle(desc.layout),
            entries: desc
                .entries
                .iter()
                .map(|entry| web::GpuBindGroupEntry {
                    binding: entry.binding,
                    resource: convert::binding_resource(&entry.resource),
                })
                .collect(),
            label: convert::label(desc.label),
        };
        DispatchBindGroup::custom(BindGroup(self.device.create_bind_group(&descriptor)))
    }

    fn create_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor<'_>,
    ) -> DispatchPipelineLayout {
        let descriptor = web::GpuPipelineLayoutDescriptor {
            bind_group_layouts: desc
                .bind_group_layouts
                .iter()
                .map(|layout| layout.map(convert::group_layout_handle))
                .collect(),
            immediate_size: (desc.immediate_size > 0).then_some(desc.immediate_size),
            label: convert::label(desc.label),
        };
        DispatchPipelineLayout::custom(PipelineLayout(self.device.create_pipeline_layout(&descriptor)))
    }

    fn create_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> DispatchRenderPipeline {
        let made = PREPARED.with(|prepared| prepared.borrow_mut().render.take());
        let pipeline = made.unwrap_or_else(|| {
            let described = render_descriptor(desc);
            self.device.create_render_pipeline(&described.descriptor)
        });
        DispatchRenderPipeline::custom(RenderPipeline(pipeline))
    }

    fn create_mesh_pipeline(
        &self,
        _desc: &wgpu::MeshPipelineDescriptor<'_>,
    ) -> DispatchRenderPipeline {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn create_compute_pipeline(
        &self,
        desc: &wgpu::ComputePipelineDescriptor<'_>,
    ) -> DispatchComputePipeline {
        let made = PREPARED.with(|prepared| prepared.borrow_mut().compute.take());
        let pipeline = made.unwrap_or_else(|| {
            let described = compute_descriptor(desc);
            self.device.create_compute_pipeline(&described.descriptor)
        });
        DispatchComputePipeline::custom(ComputePipeline(pipeline))
    }

    unsafe fn create_pipeline_cache(
        &self,
        _desc: &wgpu::PipelineCacheDescriptor<'_>,
    ) -> DispatchPipelineCache {
        unimplemented!(
            "pipeline caches are refused: apps are untrusted, and wgpu cannot validate a cache"
        )
    }

    fn create_buffer(&self, desc: &wgpu::BufferDescriptor<'_>) -> DispatchBuffer {
        let descriptor = web::GpuBufferDescriptor {
            size: desc.size,
            usage: web::GpuBufferUsage::from_bits(desc.usage.bits() & 0x3ff),
            mapped_at_creation: Some(desc.mapped_at_creation),
            label: convert::label(desc.label),
        };
        let buffer = Buffer(self.device.create_buffer(&descriptor));
        if desc.mapped_at_creation {
            FRESH.with(|fresh| fresh.borrow_mut().insert(buffer.0.handle().0));
        }
        DispatchBuffer::custom(buffer)
    }

    fn create_texture(&self, desc: &wgpu::TextureDescriptor<'_>) -> DispatchTexture {
        let descriptor = web::GpuTextureDescriptor {
            size: convert::extent(desc.size),
            mip_level_count: Some(desc.mip_level_count),
            sample_count: Some(desc.sample_count),
            dimension: Some(to_web(desc.dimension)),
            format: to_web(desc.format),
            usage: web::GpuTextureUsage::from_bits(desc.usage.bits() & 0x3f),
            view_formats: Some(desc.view_formats.iter().map(|format| to_web(*format)).collect()),
            texture_binding_view_dimension: None,
            label: convert::label(desc.label),
        };
        DispatchTexture::custom(Texture(self.device.create_texture(&descriptor)))
    }

    fn create_external_texture(
        &self,
        _desc: &wgpu::ExternalTextureDescriptor<'_>,
        _planes: &[&wgpu::TextureView],
    ) -> DispatchExternalTexture {
        unimplemented!("external textures are not available to apps yet")
    }

    fn create_blas(
        &self,
        _desc: &wgpu::CreateBlasDescriptor<'_>,
        _sizes: wgpu::BlasGeometrySizeDescriptors,
    ) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures are not in browser WebGPU")
    }

    fn create_tlas(&self, _desc: &wgpu::CreateTlasDescriptor<'_>) -> DispatchTlas {
        unimplemented!("acceleration structures are not in browser WebGPU")
    }

    fn create_sampler(&self, desc: &wgpu::SamplerDescriptor<'_>) -> DispatchSampler {
        if desc.border_color.is_some() {
            unimplemented!("border colors are not in browser WebGPU");
        }
        let descriptor = web::GpuSamplerDescriptor {
            address_mode_u: Some(to_web(desc.address_mode_u)),
            address_mode_v: Some(to_web(desc.address_mode_v)),
            address_mode_w: Some(to_web(desc.address_mode_w)),
            mag_filter: Some(to_web(desc.mag_filter)),
            min_filter: Some(to_web(desc.min_filter)),
            mipmap_filter: Some(to_web(desc.mipmap_filter)),
            lod_min_clamp: Some(desc.lod_min_clamp),
            lod_max_clamp: Some(desc.lod_max_clamp),
            compare: desc.compare.map(to_web),
            max_anisotropy: Some(desc.anisotropy_clamp),
            label: convert::label(desc.label),
        };
        DispatchSampler::custom(Sampler(self.device.create_sampler(&Some(descriptor))))
    }

    fn create_query_set(&self, desc: &wgpu::QuerySetDescriptor<'_>) -> DispatchQuerySet {
        let kind = match desc.ty {
            wgpu::QueryType::Occlusion => web::GpuQueryType::Occlusion,
            wgpu::QueryType::Timestamp => web::GpuQueryType::Timestamp,
            wgpu::QueryType::PipelineStatistics(_) => {
                unimplemented!("pipeline statistics queries are not in browser WebGPU")
            }
        };
        let descriptor = web::GpuQuerySetDescriptor {
            r#type: kind,
            count: desc.count,
            label: convert::label(desc.label),
        };
        // A query set the device's features do not allow is WebGPU's TypeError: an exception
        // in a browser, a panic here.
        let set = match self.device.create_query_set(&descriptor) {
            Ok(set) => set,
            Err(error) => {
                raise(error.message);
                web::GpuQuerySet(0)
            }
        };
        DispatchQuerySet::custom(QuerySet(gpu::GpuQuerySet::from_handle(set)))
    }

    fn create_command_encoder(
        &self,
        desc: &wgpu::CommandEncoderDescriptor<'_>,
    ) -> DispatchCommandEncoder {
        let descriptor = web::GpuCommandEncoderDescriptor {
            label: convert::label(desc.label),
        };
        DispatchCommandEncoder::custom(CommandEncoder(self.device.create_command_encoder(&Some(descriptor))))
    }

    fn create_render_bundle_encoder(
        &self,
        desc: &wgpu::RenderBundleEncoderDescriptor<'_>,
    ) -> DispatchRenderBundleEncoder {
        if desc.multiview.is_some() {
            unimplemented!("multiview render bundles are not in browser WebGPU")
        }
        let descriptor = web::GpuRenderBundleEncoderDescriptor {
            depth_read_only: desc.depth_stencil.map(|state| state.depth_read_only),
            stencil_read_only: desc.depth_stencil.map(|state| state.stencil_read_only),
            color_formats: desc
                .color_formats
                .iter()
                .map(|format| format.map(to_web))
                .collect(),
            depth_stencil_format: desc.depth_stencil.map(|state| to_web(state.format)),
            sample_count: Some(desc.sample_count),
            label: convert::label(desc.label),
        };
        DispatchRenderBundleEncoder::custom(RenderBundleEncoder(self.device.create_render_bundle_encoder(&descriptor)))
    }

    fn set_device_lost_callback(&self, device_lost_callback: BoxDeviceLostCallback) {
        let ticket = self.device.lost();
        waiting(|waiting| waiting.lost = Some((ticket, device_lost_callback)));
    }

    fn on_uncaptured_error(&self, handler: Arc<dyn wgpu::UncapturedErrorHandler>) {
        let items = self.device.on_uncaptured_error();
        waiting(|waiting| {
            waiting.handler = Some(Arc::clone(&handler));
            waiting.uncaptured = Some((items, handler));
        });
    }

    fn push_error_scope(&self, filter: wgpu::ErrorFilter) -> u32 {
        self.device.push_error_scope(match filter {
            wgpu::ErrorFilter::Validation => web::GpuErrorFilter::Validation,
            wgpu::ErrorFilter::OutOfMemory => web::GpuErrorFilter::OutOfMemory,
            wgpu::ErrorFilter::Internal => web::GpuErrorFilter::Internal,
        });
        waiting(|waiting| {
            waiting.scopes.push((filter, None));
            waiting.scopes.len() as u32 - 1
        })
    }

    /// The scope's error: one the guest raised itself first, else what the host caught.
    fn pop_error_scope(&self, _index: u32) -> Pin<Box<dyn PopErrorScopeFuture>> {
        let ticket = self.device.pop_error_scope();
        let id = waiting(|waiting| {
            waiting.next += 1;
            let raised = waiting.scopes.pop().and_then(|(_, raised)| raised);
            match raised {
                Some(error) => {
                    waiting.popped.insert(waiting.next, Some(error));
                }
                None => {
                    waiting.pops.insert(waiting.next, ticket);
                }
            }
            waiting.next
        });
        Box::pin(Settled {
            id,
            take: |waiting: &mut Waiting, id: u32| waiting.popped.remove(&id),
        })
    }

    unsafe fn start_graphics_debugger_capture(&self) {}

    unsafe fn stop_graphics_debugger_capture(&self) {}

    fn poll(
        &self,
        poll_type: wgpu::wgt::PollType<u64>,
    ) -> Result<wgpu::PollStatus, wgpu::PollError> {
        let wait = matches!(poll_type, wgpu::wgt::PollType::Wait { .. });
        Ok(match settle(wait) {
            true => wgpu::PollStatus::QueueEmpty,
            false => wgpu::PollStatus::Poll,
        })
    }

    fn get_internal_counters(&self) -> wgpu::InternalCounters {
        wgpu::InternalCounters::default()
    }

    fn generate_allocator_report(&self) -> Option<wgpu::AllocatorReport> {
        None
    }

    fn destroy(&self) {
        self.device.destroy();
    }
}

impl QueueInterface for Queue {
    fn write_buffer(&self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress, data: &[u8]) {
        // The data is whole, so only the host's own checks can refuse it, into the device's
        // error scopes.
        let _ = self.0.write_buffer_with_copy(&custom::<Buffer>(buffer.as_custom()).0, offset, data, None, None);
    }

    fn create_staging_buffer(&self, size: wgpu::BufferSize) -> Option<DispatchQueueWriteBuffer> {
        Some(DispatchQueueWriteBuffer::custom(StagingBuffer {
            bytes: vec![0u8; size.get() as usize],
        }))
    }

    fn validate_write_buffer(
        &self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _size: wgpu::BufferSize,
    ) -> Option<()> {
        Some(())
    }

    fn write_staging_buffer(
        &self,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        staging_buffer: &DispatchQueueWriteBuffer,
    ) {
        let Some(staging) = staging_buffer.as_custom::<StagingBuffer>() else {
            panic!("a staging buffer from another wgpu backend reached wasi:webgpu");
        };
        self.write_buffer(buffer, offset, &staging.bytes);
    }

    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        data_layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        self.0.write_texture_with_copy(
            &convert::texel_copy_texture(texture),
            data,
            convert::texel_copy_buffer_layout(data_layout),
            convert::extent(size),
        );
    }

    #[cfg(target_arch = "wasm32")]
    fn copy_external_image_to_texture(
        &self,
        _source: &wgpu::CopyExternalImageSourceInfo,
        _dest: wgpu::CopyExternalImageDestInfo<&wgpu::Texture>,
        _size: wgpu::Extent3d,
    ) {
        unimplemented!("external image copies are not available to apps yet")
    }

    fn submit(&self, command_buffers: &mut dyn Iterator<Item = DispatchCommandBuffer>) -> u64 {
        let buffers: Vec<DispatchCommandBuffer> = command_buffers.collect();
        let owned: Vec<&gpu::GpuCommandBuffer> = buffers
            .iter()
            .map(|buffer| &custom::<CommandBuffer>(buffer.as_custom()).0)
            .collect();
        self.0.submit(&owned);
        0
    }

    fn get_timestamp_period(&self) -> f32 {
        1.0
    }

    fn on_submitted_work_done(&self, callback: BoxSubmittedWorkDoneCallback) {
        let ticket = self.0.on_submitted_work_done();
        waiting(|waiting| waiting.work.push((ticket, callback)));
    }

    fn compact_blas(&self, _blas: &DispatchBlas) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures are not in browser WebGPU")
    }

    fn present(&self, _detail: &DispatchSurfaceOutputDetail) {
        unreachable!("an app's surfaces are its host's, presented with `present_surface`")
    }
}

impl ShaderModuleInterface for ShaderModule {
    fn get_compilation_info(&self) -> Pin<Box<dyn ShaderCompilationInfoFuture>> {
        let ticket = self.0.get_compilation_info();
        let id = waiting(|waiting| {
            waiting.next += 1;
            waiting.compiling.insert(waiting.next, ticket);
            waiting.next
        });
        Box::pin(Settled {
            id,
            take: |waiting: &mut Waiting, id: u32| waiting.compiled.remove(&id),
        })
    }
}

impl BindGroupLayoutInterface for BindGroupLayout {}
impl BindGroupInterface for BindGroup {}
impl TextureViewInterface for TextureView {}
impl SamplerInterface for Sampler {}
impl PipelineLayoutInterface for PipelineLayout {}
impl CommandBufferInterface for CommandBuffer {}
impl QuerySetInterface for QuerySet {
    fn destroy(&self) {
        self.0.destroy();
    }
}
impl RenderBundleInterface for RenderBundle {}

impl RenderPipelineInterface for RenderPipeline {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        DispatchBindGroupLayout::custom(BindGroupLayout(self.0.get_bind_group_layout(index)))
    }
}

impl ComputePipelineInterface for ComputePipeline {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        DispatchBindGroupLayout::custom(BindGroupLayout(self.0.get_bind_group_layout(index)))
    }
}

impl BufferInterface for Buffer {
    fn map_async(
        &self,
        mode: wgpu::MapMode,
        range: Range<wgpu::BufferAddress>,
        callback: BufferMapCallback,
    ) {
        let mode = match mode {
            wgpu::MapMode::Read => web::GpuMapMode::READ,
            wgpu::MapMode::Write => web::GpuMapMode::WRITE,
        };
        let ticket = self.0.map_async(
            mode,
            Some(range.start),
            Some(range.end.saturating_sub(range.start)),
        );
        waiting(|waiting| waiting.maps.push((ticket, self.0.handle(), callback)));
    }

    /// Copies the range out of the host's map once. A buffer mapped at creation starts zeroed
    /// instead, and a range written through `get_mapped_range_mut` goes back to the host when
    /// dropped.
    fn get_mapped_range(
        &self,
        sub_range: Range<wgpu::BufferAddress>,
    ) -> Result<DispatchBufferMappedRange, wgpu::MapRangeError> {
        let length = sub_range.end.saturating_sub(sub_range.start);
        let fresh = FRESH.with(|fresh| fresh.borrow().contains(&self.0.handle().0));
        let bytes = match fresh {
            true => vec![0u8; length as usize],
            false => self
                .0
                .get_mapped_range_get_with_copy(Some(sub_range.start), Some(length))
                .unwrap_or_else(|error| panic!("{}", error.message)),
        };
        Ok(DispatchBufferMappedRange::custom(MappedRange {
            buffer: self.0.handle(),
            offset: sub_range.start,
            bytes,
            written: false,
        }))
    }

    fn unmap(&self) {
        FRESH.with(|fresh| fresh.borrow_mut().remove(&self.0.handle().0));
        abort_maps(self.0.handle());
        let _ = self.0.unmap();
    }

    fn destroy(&self) {
        FRESH.with(|fresh| fresh.borrow_mut().remove(&self.0.handle().0));
        abort_maps(self.0.handle());
        self.0.destroy();
    }
}

impl TextureInterface for Texture {
    fn create_view(&self, desc: &wgpu::TextureViewDescriptor<'_>) -> DispatchTextureView {
        let descriptor = web::GpuTextureViewDescriptor {
            format: desc.format.map(to_web),
            dimension: desc.dimension.map(to_web),
            usage: desc
                .usage
                .map(|usage| web::GpuTextureUsage::from_bits(usage.bits() & 0x3f)),
            aspect: Some(to_web(desc.aspect)),
            base_mip_level: Some(desc.base_mip_level),
            mip_level_count: desc.mip_level_count,
            base_array_layer: Some(desc.base_array_layer),
            array_layer_count: desc.array_layer_count,
            swizzle: None,
            label: convert::label(desc.label),
        };
        DispatchTextureView::custom(TextureView(self.0.create_view(&Some(descriptor))))
    }

    fn destroy(&self) {
        self.0.destroy();
    }
}

// unsafe(ffi)
impl QueueWriteBufferInterface for StagingBuffer {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        wgpu::WriteOnly::from_mut(&mut self.bytes)
    }
}

// unsafe(ffi)
impl BufferMappedRangeInterface for MappedRange {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    unsafe fn read_slice(&self) -> &[u8] {
        &self.bytes
    }

    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        self.written = true;
        wgpu::WriteOnly::from_mut(&mut self.bytes)
    }
}
