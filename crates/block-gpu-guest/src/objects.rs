use core::{fmt, future::ready, mem::ManuallyDrop, ops::Range, pin::Pin};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use block_gpu_abi::{device as abi, guest::device as gpu};
use wgpu::custom::*;

use crate::convert::{self, custom};
use crate::features::Allowed;

mod compute;

pub(crate) fn last_error() -> String {
    gpu::take_error().unwrap_or_default()
}

struct Scope {
    filter: wgpu::ErrorFilter,
    error: Option<wgpu::Error>,
}

struct Map {
    buffer: abi::Buffer,
    callback: BufferMapCallback,
}

thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
    static MAPS: RefCell<Vec<Map>> = const { RefCell::new(Vec::new()) };
    static FRESH: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
    static UNCAPTURED: RefCell<Option<Arc<dyn wgpu::UncapturedErrorHandler>>> =
        const { RefCell::new(None) };
}

#[derive(Debug)]
struct Rejected(String);

impl fmt::Display for Rejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Rejected {}

fn reject(message: String) {
    let error = wgpu::Error::Validation {
        source: Box::new(Rejected(message.clone())),
        description: message,
    };
    let unscoped = SCOPES.with(|scopes| {
        let mut scopes = scopes.borrow_mut();
        match scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.filter == wgpu::ErrorFilter::Validation)
        {
            Some(scope) => {
                scope.error.get_or_insert(error);
                None
            }
            None => Some(error),
        }
    });
    let Some(error) = unscoped else {
        return;
    };
    match UNCAPTURED.with(|handler| handler.borrow().clone()) {
        Some(handler) => handler(error),
        None => panic!("the plugin gpu abi rejected a request: {error}"),
    }
}

fn checked() {
    if SCOPES.with(|scopes| scopes.borrow().is_empty()) {
        return;
    }
    let message = last_error();
    if !message.is_empty() {
        reject(message);
    }
}

pub(crate) fn settle(wait: bool) -> bool {
    if MAPS.with(|maps| maps.borrow().is_empty()) {
        return true;
    }
    gpu::poll(wait);
    let mut states = HashMap::new();
    let settled: Vec<(Map, abi::MapState)> = MAPS.with(|maps| {
        let mut maps = maps.borrow_mut();
        let mut settled = Vec::new();
        let mut index = 0;
        while index < maps.len() {
            let buffer = maps[index].buffer;
            let state = *states
                .entry(buffer.0)
                .or_insert_with(|| ManuallyDrop::new(gpu::Buffer::from_handle(buffer)).map_state());
            match state {
                abi::MapState::Pending => index += 1,
                _ => settled.push((maps.remove(index), state)),
            }
        }
        settled
    });
    for (map, state) in settled {
        (map.callback)(match state {
            abi::MapState::Mapped => Ok(()),
            _ => Err(wgpu::BufferAsyncError),
        });
    }
    MAPS.with(|maps| maps.borrow().is_empty())
}

fn abort_maps(buffer: abi::Buffer) {
    let aborted: Vec<Map> = MAPS.with(|maps| {
        let mut maps = maps.borrow_mut();
        let (aborted, kept) = maps.drain(..).partition(|map| map.buffer == buffer);
        *maps = kept;
        aborted
    });
    for map in aborted {
        (map.callback)(Err(wgpu::BufferAsyncError));
    }
}

macro_rules! resource {
    ($name:ident) => {
        #[derive(Debug)]
        pub(crate) struct $name(pub(crate) gpu::$name);

        impl $name {
            fn created(made: Option<gpu::$name>) -> Self {
                Self(made.unwrap_or_else(|| {
                    reject(last_error());
                    gpu::$name::from_handle(abi::$name(0))
                }))
            }

            #[allow(dead_code)]
            fn refused(message: String) -> Self {
                reject(message);
                Self(gpu::$name::from_handle(abi::$name(0)))
            }
        }
    };
}

pub(crate) use resource;

resource!(Buffer);
resource!(Texture);
resource!(TextureView);
resource!(Sampler);
resource!(BindGroupLayout);
resource!(BindGroup);
resource!(PipelineLayout);
resource!(ShaderModule);
resource!(RenderPipeline);
resource!(CommandBuffer);
resource!(CommandEncoder);
resource!(RenderPass);
resource!(QuerySet);
resource!(RenderBundleEncoder);
resource!(RenderBundle);

impl Drop for Buffer {
    fn drop(&mut self) {
        let handle = self.0.handle();
        FRESH.with(|fresh| fresh.borrow_mut().remove(&handle.0));
        abort_maps(handle);
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        self.0.end();
        checked();
    }
}

#[derive(Debug)]
pub(crate) struct StagingBuffer {
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct MappedRange {
    buffer: abi::Buffer,
    offset: u64,
    bytes: Vec<u8>,
    written: bool,
}

impl Drop for MappedRange {
    fn drop(&mut self) {
        if !self.written {
            return;
        }
        let buffer = ManuallyDrop::new(gpu::Buffer::from_handle(self.buffer));
        buffer.write_mapped(self.offset, &self.bytes);
    }
}

#[derive(Debug)]
pub(crate) struct Device {
    allowed: Allowed,
    limits: wgpu::Limits,
}

#[derive(Debug)]
pub(crate) struct Queue;

impl Device {
    pub(crate) fn new(
        features: wgpu::Features,
        offered: wgpu::Features,
        limits: wgpu::Limits,
    ) -> Self {
        Self {
            allowed: Allowed {
                requested: features,
                offered,
            },
            limits,
        }
    }
}

// unsafe(ffi)
impl DeviceInterface for Device {
    fn features(&self) -> wgpu::Features {
        self.allowed.requested
    }

    fn limits(&self) -> wgpu::Limits {
        self.limits.clone()
    }

    fn adapter_info(&self) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "BE3 plugin host".into(),
            vendor: 0,
            device: 0,
            device_type: wgpu::DeviceType::Other,
            device_pci_bus_id: String::new(),
            driver: "be3".into(),
            driver_info: String::new(),
            backend: wgpu::Backend::Noop,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: false,
        }
    }

    fn create_shader_module(
        &self,
        desc: wgpu::ShaderModuleDescriptor<'_>,
        _shader_bound_checks: wgpu::ShaderRuntimeChecks,
    ) -> DispatchShaderModule {
        let wgsl = match desc.source {
            wgpu::ShaderSource::Wgsl(source) => source.into_owned(),
            _ => panic!("the plugin gpu abi only accepts wgsl shaders, as browser WebGPU does"),
        };
        if let Err(message) = self.allowed.shader(&wgsl) {
            return DispatchShaderModule::custom(ShaderModule::refused(message));
        }
        let descriptor = abi::ShaderModuleDescriptor {
            label: convert::label(desc.label),
            wgsl,
        };
        DispatchShaderModule::custom(ShaderModule::created(gpu::create_shader_module(
            &descriptor,
        )))
    }

    unsafe fn create_shader_module_passthrough(
        &self,
        _desc: &wgpu::ShaderModuleDescriptorPassthrough<'_>,
    ) -> DispatchShaderModule {
        unimplemented!(
            "passthrough shaders are refused: plugins are untrusted, and wgpu cannot validate them"
        )
    }

    fn create_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> DispatchBindGroupLayout {
        if let Err(message) = self.allowed.group_layout(desc) {
            return DispatchBindGroupLayout::custom(BindGroupLayout::refused(message));
        }
        let descriptor = abi::BindGroupLayoutDescriptor {
            label: convert::label(desc.label),
            entries: desc
                .entries
                .iter()
                .map(|entry| abi::BindGroupLayoutEntry {
                    binding: entry.binding,
                    visibility: entry.visibility.bits(),
                    r#type: convert::binding_type(entry.ty),
                    count: entry.count.map(|count| count.get()),
                })
                .collect(),
        };
        DispatchBindGroupLayout::custom(BindGroupLayout::created(gpu::create_bind_group_layout(
            &descriptor,
        )))
    }

    fn create_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> DispatchBindGroup {
        let descriptor = abi::BindGroupDescriptor {
            label: convert::label(desc.label),
            layout: convert::group_layout_handle(desc.layout),
            entries: desc
                .entries
                .iter()
                .map(|entry| abi::BindGroupEntry {
                    binding: entry.binding,
                    resource: convert::binding_resource(&entry.resource),
                })
                .collect(),
        };
        DispatchBindGroup::custom(BindGroup::created(gpu::create_bind_group(&descriptor)))
    }

    fn create_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor<'_>,
    ) -> DispatchPipelineLayout {
        let descriptor = abi::PipelineLayoutDescriptor {
            label: convert::label(desc.label),
            bind_group_layouts: desc
                .bind_group_layouts
                .iter()
                .map(|layout| layout.map(convert::group_layout_handle))
                .collect(),
        };
        DispatchPipelineLayout::custom(PipelineLayout::created(gpu::create_pipeline_layout(
            &descriptor,
        )))
    }

    fn create_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> DispatchRenderPipeline {
        if let Err(message) = self.allowed.render_pipeline(desc) {
            return DispatchRenderPipeline::custom(RenderPipeline::refused(message));
        }
        let descriptor = abi::RenderPipelineDescriptor {
            label: convert::label(desc.label),
            layout: desc.layout.map(convert::layout_handle),
            vertex: abi::VertexState {
                module: convert::module_handle(desc.vertex.module),
                entry_point: desc.vertex.entry_point.map(str::to_owned),
                buffers: desc
                    .vertex
                    .buffers
                    .iter()
                    .map(|buffer| abi::VertexBufferLayout {
                        array_stride: buffer.array_stride,
                        step_mode: convert::step_mode(buffer.step_mode),
                        attributes: buffer
                            .attributes
                            .iter()
                            .map(|attribute| abi::VertexAttribute {
                                format: convert::vertex_format(attribute.format),
                                offset: attribute.offset,
                                shader_location: attribute.shader_location,
                            })
                            .collect(),
                    })
                    .collect(),
            },
            primitive: abi::PrimitiveState {
                topology: convert::topology(desc.primitive.topology),
                strip_index_format: desc.primitive.strip_index_format.map(convert::index_format),
                front_face: convert::front_face(desc.primitive.front_face),
                cull_mode: desc.primitive.cull_mode.map(convert::face),
                unclipped_depth: desc.primitive.unclipped_depth,
                polygon_mode: convert::polygon_mode(desc.primitive.polygon_mode),
                conservative: desc.primitive.conservative,
            },
            depth_stencil: desc
                .depth_stencil
                .as_ref()
                .map(|state| abi::DepthStencilState {
                    format: convert::texture_format(state.format),
                    depth_write_enabled: state.depth_write_enabled.unwrap_or(false),
                    depth_compare: state.depth_compare.map(convert::compare_function),
                    stencil: abi::StencilState {
                        front: convert::stencil_face(state.stencil.front),
                        back: convert::stencil_face(state.stencil.back),
                        read_mask: state.stencil.read_mask,
                        write_mask: state.stencil.write_mask,
                    },
                    bias: abi::DepthBiasState {
                        constant: state.bias.constant,
                        slope_scale: state.bias.slope_scale,
                        clamp: state.bias.clamp,
                    },
                }),
            multisample: abi::MultisampleState {
                count: desc.multisample.count,
                mask: desc.multisample.mask,
                alpha_to_coverage_enabled: desc.multisample.alpha_to_coverage_enabled,
            },
            fragment: desc.fragment.as_ref().map(|fragment| abi::FragmentState {
                module: convert::module_handle(fragment.module),
                entry_point: fragment.entry_point.map(str::to_owned),
                targets: fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.as_ref().map(|target| abi::ColorTargetState {
                            format: convert::texture_format(target.format),
                            blend: target.blend.map(|blend| abi::BlendState {
                                color: convert::blend_component(blend.color),
                                alpha: convert::blend_component(blend.alpha),
                            }),
                            write_mask: target.write_mask.bits(),
                        })
                    })
                    .collect(),
            }),
        };
        DispatchRenderPipeline::custom(RenderPipeline::created(gpu::create_render_pipeline(
            &descriptor,
        )))
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
        compute::pipeline(desc)
    }

    unsafe fn create_pipeline_cache(
        &self,
        _desc: &wgpu::PipelineCacheDescriptor<'_>,
    ) -> DispatchPipelineCache {
        unimplemented!(
            "pipeline caches are refused: plugins are untrusted, and wgpu cannot validate a cache"
        )
    }

    fn create_buffer(&self, desc: &wgpu::BufferDescriptor<'_>) -> DispatchBuffer {
        let descriptor = abi::BufferDescriptor {
            label: convert::label(desc.label),
            size: desc.size,
            usage: desc.usage.bits(),
            mapped_at_creation: desc.mapped_at_creation,
        };
        let buffer = Buffer::created(gpu::create_buffer(&descriptor));
        if desc.mapped_at_creation {
            FRESH.with(|fresh| fresh.borrow_mut().insert(buffer.0.handle().0));
        }
        DispatchBuffer::custom(buffer)
    }

    fn create_texture(&self, desc: &wgpu::TextureDescriptor<'_>) -> DispatchTexture {
        if let Err(message) = self.allowed.texture(desc) {
            return DispatchTexture::custom(Texture::refused(message));
        }
        let descriptor = abi::TextureDescriptor {
            label: convert::label(desc.label),
            size: convert::extent(desc.size),
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: convert::texture_dimension(desc.dimension),
            format: convert::texture_format(desc.format),
            usage: desc.usage.bits(),
            view_formats: desc
                .view_formats
                .iter()
                .map(|format| convert::texture_format(*format))
                .collect(),
        };
        DispatchTexture::custom(Texture::created(gpu::create_texture(&descriptor)))
    }

    fn create_external_texture(
        &self,
        _desc: &wgpu::ExternalTextureDescriptor<'_>,
        _planes: &[&wgpu::TextureView],
    ) -> DispatchExternalTexture {
        unimplemented!("external textures are not available to plugins yet")
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
        let descriptor = abi::SamplerDescriptor {
            label: convert::label(desc.label),
            address_mode_u: convert::address_mode(desc.address_mode_u),
            address_mode_v: convert::address_mode(desc.address_mode_v),
            address_mode_w: convert::address_mode(desc.address_mode_w),
            mag_filter: convert::filter_mode(desc.mag_filter),
            min_filter: convert::filter_mode(desc.min_filter),
            mipmap_filter: convert::mipmap_filter_mode(desc.mipmap_filter),
            lod_min_clamp: desc.lod_min_clamp,
            lod_max_clamp: desc.lod_max_clamp,
            compare: desc.compare.map(convert::compare_function),
            anisotropy_clamp: desc.anisotropy_clamp,
            border_color: desc.border_color.map(convert::border_color),
        };
        DispatchSampler::custom(Sampler::created(gpu::create_sampler(&descriptor)))
    }

    fn create_query_set(&self, desc: &wgpu::QuerySetDescriptor<'_>) -> DispatchQuerySet {
        let kind = match desc.ty {
            wgpu::QueryType::Occlusion => abi::QueryType::Occlusion,
            wgpu::QueryType::Timestamp => abi::QueryType::Timestamp,
            wgpu::QueryType::PipelineStatistics(_) => {
                unimplemented!("pipeline statistics queries are not in browser WebGPU")
            }
        };
        if let Err(message) = self.allowed.query_set(desc.ty) {
            return DispatchQuerySet::custom(QuerySet::refused(message));
        }
        let descriptor = abi::QuerySetDescriptor {
            label: convert::label(desc.label),
            r#type: kind,
            count: desc.count,
        };
        DispatchQuerySet::custom(QuerySet::created(gpu::create_query_set(&descriptor)))
    }

    fn create_command_encoder(
        &self,
        desc: &wgpu::CommandEncoderDescriptor<'_>,
    ) -> DispatchCommandEncoder {
        let descriptor = abi::CommandEncoderDescriptor {
            label: convert::label(desc.label),
        };
        DispatchCommandEncoder::custom(CommandEncoder::created(gpu::create_command_encoder(
            &descriptor,
        )))
    }

    fn create_render_bundle_encoder(
        &self,
        desc: &wgpu::RenderBundleEncoderDescriptor<'_>,
    ) -> DispatchRenderBundleEncoder {
        if desc.multiview.is_some() {
            unimplemented!("multiview render bundles are not in browser WebGPU")
        }
        let descriptor = abi::RenderBundleEncoderDescriptor {
            label: convert::label(desc.label),
            color_formats: desc
                .color_formats
                .iter()
                .map(|format| format.map(convert::texture_format))
                .collect(),
            depth_stencil: desc
                .depth_stencil
                .map(|state| abi::RenderBundleDepthStencil {
                    format: convert::texture_format(state.format),
                    depth_read_only: state.depth_read_only,
                    stencil_read_only: state.stencil_read_only,
                }),
            sample_count: desc.sample_count,
        };
        DispatchRenderBundleEncoder::custom(RenderBundleEncoder::created(
            gpu::create_render_bundle_encoder(&descriptor),
        ))
    }

    fn set_device_lost_callback(&self, _device_lost_callback: BoxDeviceLostCallback) {}

    fn on_uncaptured_error(&self, handler: Arc<dyn wgpu::UncapturedErrorHandler>) {
        UNCAPTURED.with(|held| *held.borrow_mut() = Some(handler));
    }

    fn push_error_scope(&self, filter: wgpu::ErrorFilter) -> u32 {
        SCOPES.with(|scopes| {
            let mut scopes = scopes.borrow_mut();
            scopes.push(Scope {
                filter,
                error: None,
            });
            scopes.len() as u32 - 1
        })
    }

    fn pop_error_scope(&self, index: u32) -> Pin<Box<dyn PopErrorScopeFuture>> {
        let error = SCOPES.with(|scopes| {
            let mut scopes = scopes.borrow_mut();
            let popped = scopes.drain(index as usize..).next();
            popped.and_then(|scope| scope.error)
        });
        Box::pin(ready(error))
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

    fn destroy(&self) {}
}

impl QueueInterface for Queue {
    fn write_buffer(&self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress, data: &[u8]) {
        gpu::write_buffer(&custom::<Buffer>(buffer.as_custom()).0, offset, data);
        checked();
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
            panic!("a staging buffer from another wgpu backend reached the plugin gpu abi");
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
        gpu::write_texture(
            convert::texel_copy_texture(texture),
            data,
            convert::texel_copy_buffer_layout(data_layout),
            convert::extent(size),
        );
        checked();
    }

    #[cfg(target_arch = "wasm32")]
    fn copy_external_image_to_texture(
        &self,
        _source: &wgpu::CopyExternalImageSourceInfo,
        _dest: wgpu::CopyExternalImageDestInfo<&wgpu::Texture>,
        _size: wgpu::Extent3d,
    ) {
        unimplemented!("external image copies are not available to plugins yet")
    }

    fn submit(&self, command_buffers: &mut dyn Iterator<Item = DispatchCommandBuffer>) -> u64 {
        let buffers: Vec<gpu::CommandBuffer> = command_buffers
            .map(|buffer| {
                let handle = custom::<CommandBuffer>(buffer.as_custom()).0.handle();
                std::mem::forget(buffer);
                gpu::CommandBuffer::from_handle(handle)
            })
            .collect();
        gpu::submit(buffers);
        checked();
        0
    }

    fn get_timestamp_period(&self) -> f32 {
        1.0
    }

    fn on_submitted_work_done(&self, callback: BoxSubmittedWorkDoneCallback) {
        callback();
    }

    fn compact_blas(&self, _blas: &DispatchBlas) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures are not in browser WebGPU")
    }
}

impl ShaderModuleInterface for ShaderModule {
    fn get_compilation_info(&self) -> Pin<Box<dyn ShaderCompilationInfoFuture>> {
        Box::pin(ready(wgpu::CompilationInfo {
            messages: Vec::new(),
        }))
    }
}

impl BindGroupLayoutInterface for BindGroupLayout {}
impl BindGroupInterface for BindGroup {}
impl TextureViewInterface for TextureView {}
impl SamplerInterface for Sampler {}
impl PipelineLayoutInterface for PipelineLayout {}
impl CommandBufferInterface for CommandBuffer {}
impl QuerySetInterface for QuerySet {}
impl RenderBundleInterface for RenderBundle {}

impl RenderPipelineInterface for RenderPipeline {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        DispatchBindGroupLayout::custom(BindGroupLayout::created(
            self.0.get_bind_group_layout(index),
        ))
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
            wgpu::MapMode::Read => abi::MapMode::Read,
            wgpu::MapMode::Write => abi::MapMode::Write,
        };
        self.0
            .map_async(mode, range.start, range.end.saturating_sub(range.start));
        checked();
        MAPS.with(|maps| {
            maps.borrow_mut().push(Map {
                buffer: self.0.handle(),
                callback,
            })
        });
    }

    fn get_mapped_range(&self, sub_range: Range<wgpu::BufferAddress>) -> DispatchBufferMappedRange {
        let length = sub_range.end.saturating_sub(sub_range.start);
        let fresh = FRESH.with(|fresh| fresh.borrow().contains(&self.0.handle().0));
        let bytes = match fresh {
            true => vec![0u8; length as usize],
            false => self.0.read_mapped(sub_range.start, length),
        };
        checked();
        DispatchBufferMappedRange::custom(MappedRange {
            buffer: self.0.handle(),
            offset: sub_range.start,
            bytes,
            written: false,
        })
    }

    fn unmap(&self) {
        FRESH.with(|fresh| fresh.borrow_mut().remove(&self.0.handle().0));
        abort_maps(self.0.handle());
        self.0.unmap();
        checked();
    }

    fn destroy(&self) {}
}

impl TextureInterface for Texture {
    fn create_view(&self, desc: &wgpu::TextureViewDescriptor<'_>) -> DispatchTextureView {
        let descriptor = abi::TextureViewDescriptor {
            label: convert::label(desc.label),
            format: desc.format.map(convert::texture_format),
            dimension: desc.dimension.map(convert::texture_view_dimension),
            aspect: convert::texture_aspect(desc.aspect),
            base_mip_level: desc.base_mip_level,
            mip_level_count: desc.mip_level_count,
            base_array_layer: desc.base_array_layer,
            array_layer_count: desc.array_layer_count,
        };
        DispatchTextureView::custom(TextureView::created(self.0.create_view(&descriptor)))
    }

    fn destroy(&self) {}
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

impl CommandEncoderInterface for CommandEncoder {
    fn copy_buffer_to_buffer(
        &self,
        source: &DispatchBuffer,
        source_offset: wgpu::BufferAddress,
        destination: &DispatchBuffer,
        destination_offset: wgpu::BufferAddress,
        copy_size: Option<wgpu::BufferAddress>,
    ) {
        self.0.copy_buffer_to_buffer(
            &custom::<Buffer>(source.as_custom()).0,
            source_offset,
            &custom::<Buffer>(destination.as_custom()).0,
            destination_offset,
            copy_size,
        );
        checked();
    }

    fn copy_buffer_to_texture(
        &self,
        source: wgpu::TexelCopyBufferInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_buffer_to_texture(
            convert::texel_copy_buffer(source),
            convert::texel_copy_texture(destination),
            convert::extent(copy_size),
        );
        checked();
    }

    fn copy_texture_to_buffer(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyBufferInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_texture_to_buffer(
            convert::texel_copy_texture(source),
            convert::texel_copy_buffer(destination),
            convert::extent(copy_size),
        );
        checked();
    }

    fn copy_texture_to_texture(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_texture_to_texture(
            convert::texel_copy_texture(source),
            convert::texel_copy_texture(destination),
            convert::extent(copy_size),
        );
        checked();
    }

    fn begin_compute_pass(&self, desc: &wgpu::ComputePassDescriptor<'_>) -> DispatchComputePass {
        compute::pass(&self.0, desc)
    }

    fn begin_render_pass(&self, desc: &wgpu::RenderPassDescriptor<'_>) -> DispatchRenderPass {
        let descriptor = abi::RenderPassDescriptor {
            label: convert::label(desc.label),
            color_attachments: desc
                .color_attachments
                .iter()
                .map(|attachment| {
                    attachment.as_ref().map(|attachment| abi::ColorAttachment {
                        view: convert::view_handle(attachment.view),
                        resolve_target: attachment.resolve_target.map(convert::view_handle),
                        load: match attachment.ops.load {
                            wgpu::LoadOp::Clear(value) => {
                                abi::ColorLoadOp::Clear(convert::color(value))
                            }
                            wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => {
                                abi::ColorLoadOp::Load
                            }
                        },
                        store: convert::store_op(attachment.ops.store),
                        depth_slice: attachment.depth_slice,
                    })
                })
                .collect(),
            depth_stencil_attachment: desc.depth_stencil_attachment.as_ref().map(|attachment| {
                abi::DepthStencilAttachment {
                    view: convert::view_handle(attachment.view),
                    depth_load: attachment.depth_ops.map(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => abi::DepthLoadOp::Clear(value),
                        wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => abi::DepthLoadOp::Load,
                    }),
                    depth_store: attachment
                        .depth_ops
                        .map_or(abi::StoreOp::Store, |ops| convert::store_op(ops.store)),
                    depth_read_only: attachment.depth_ops.is_none(),
                    stencil_load: attachment.stencil_ops.map(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => abi::StencilLoadOp::Clear(value),
                        wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => abi::StencilLoadOp::Load,
                    }),
                    stencil_store: attachment
                        .stencil_ops
                        .map_or(abi::StoreOp::Store, |ops| convert::store_op(ops.store)),
                    stencil_read_only: attachment.stencil_ops.is_none(),
                }
            }),
            occlusion_query_set: desc
                .occlusion_query_set
                .map(|set| custom::<QuerySet>(set.as_custom()).0.handle()),
            timestamp_writes: desc.timestamp_writes.as_ref().map(|writes| {
                abi::PassTimestampWrites {
                    query_set: custom::<QuerySet>(writes.query_set.as_custom()).0.handle(),
                    beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                    end_of_pass_write_index: writes.end_of_pass_write_index,
                }
            }),
        };
        DispatchRenderPass::custom(RenderPass::created(self.0.begin_render_pass(&descriptor)))
    }

    fn finish(&mut self) -> DispatchCommandBuffer {
        DispatchCommandBuffer::custom(CommandBuffer::created(self.0.finish()))
    }

    fn clear_texture(
        &self,
        _texture: &DispatchTexture,
        _subresource_range: &wgpu::ImageSubresourceRange,
    ) {
        unimplemented!("texture clears are not in browser WebGPU")
    }

    fn clear_buffer(
        &self,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferAddress>,
    ) {
        self.0
            .clear_buffer(&custom::<Buffer>(buffer.as_custom()).0, offset, size);
        checked();
    }

    fn insert_debug_marker(&self, _label: &str) {}

    fn push_debug_group(&self, _label: &str) {}

    fn pop_debug_group(&self) {}

    fn write_timestamp(&self, _query_set: &DispatchQuerySet, _query_index: u32) {
        unimplemented!("timestamps written inside command encoders are not in browser WebGPU")
    }

    fn resolve_query_set(
        &self,
        query_set: &DispatchQuerySet,
        first_query: u32,
        query_count: u32,
        destination: &DispatchBuffer,
        destination_offset: wgpu::BufferAddress,
    ) {
        self.0.resolve_query_set(
            &custom::<QuerySet>(query_set.as_custom()).0,
            first_query,
            query_count,
            &custom::<Buffer>(destination.as_custom()).0,
            destination_offset,
        );
        checked();
    }

    fn mark_acceleration_structures_built<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::Blas>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
    }

    fn build_acceleration_structures<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::BlasBuildEntry<'a>>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
    }

    fn transition_resources<'a>(
        &mut self,
        _buffer_transitions: &mut dyn Iterator<Item = wgpu::BufferTransition<&'a DispatchBuffer>>,
        _texture_transitions: &mut dyn Iterator<
            Item = wgpu::TextureTransition<&'a DispatchTexture>,
        >,
    ) {
    }
}

impl RenderPassInterface for RenderPass {
    fn set_pipeline(&mut self, pipeline: &DispatchRenderPipeline) {
        self.0
            .set_pipeline(&custom::<RenderPipeline>(pipeline.as_custom()).0);
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: Option<&DispatchBindGroup>,
        offsets: &[wgpu::DynamicOffset],
    ) {
        let group = bind_group.map(|group| &custom::<BindGroup>(group.as_custom()).0);
        self.0.set_bind_group(index, group, offsets);
    }

    fn set_index_buffer(
        &mut self,
        buffer: &DispatchBuffer,
        index_format: wgpu::IndexFormat,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_index_buffer(
            &custom::<Buffer>(buffer.as_custom()).0,
            convert::index_format(index_format),
            offset,
            convert::size(size),
        );
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_vertex_buffer(
            slot,
            &custom::<Buffer>(buffer.as_custom()).0,
            offset,
            convert::size(size),
        );
    }

    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediate data is not in browser WebGPU")
    }

    fn set_blend_constant(&mut self, color: wgpu::Color) {
        self.0.set_blend_constant(convert::color(color));
    }

    fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.0.set_scissor_rect(x, y, width, height);
    }

    fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.0
            .set_viewport(x, y, width, height, min_depth, max_depth);
    }

    fn set_stencil_reference(&mut self, reference: u32) {
        self.0.set_stencil_reference(reference);
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.0.draw(
            vertices.end.saturating_sub(vertices.start),
            instances.end.saturating_sub(instances.start),
            vertices.start,
            instances.start,
        );
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.0.draw_indexed(
            indices.end.saturating_sub(indices.start),
            instances.end.saturating_sub(instances.start),
            indices.start,
            base_vertex,
            instances.start,
        );
    }

    fn draw_mesh_tasks(&mut self, _x: u32, _y: u32, _z: u32) {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn draw_indirect(&mut self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0
            .draw_indirect(&custom::<Buffer>(buffer.as_custom()).0, offset);
    }

    fn draw_indexed_indirect(&mut self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0
            .draw_indexed_indirect(&custom::<Buffer>(buffer.as_custom()).0, offset);
    }

    fn draw_mesh_tasks_indirect(&mut self, _buffer: &DispatchBuffer, _offset: wgpu::BufferAddress) {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn multi_draw_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("multi-draw-indirect is not in browser WebGPU")
    }

    fn multi_draw_indexed_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("multi-draw-indirect is not in browser WebGPU")
    }

    fn multi_draw_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("multi-draw-indirect counts are not in browser WebGPU")
    }

    fn multi_draw_mesh_tasks_indirect(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count: u32,
    ) {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn multi_draw_indexed_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("multi-draw-indirect counts are not in browser WebGPU")
    }

    fn multi_draw_mesh_tasks_indirect_count(
        &mut self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _count_buffer: &DispatchBuffer,
        _count_buffer_offset: wgpu::BufferAddress,
        _max_count: u32,
    ) {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn insert_debug_marker(&mut self, _label: &str) {}

    fn push_debug_group(&mut self, _group_label: &str) {}

    fn pop_debug_group(&mut self) {}

    fn write_timestamp(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {
        unimplemented!(
            "timestamps written inside passes are not in browser WebGPU: use the pass's timestamp writes"
        )
    }

    fn begin_occlusion_query(&mut self, query_index: u32) {
        self.0.begin_occlusion_query(query_index);
    }

    fn end_occlusion_query(&mut self) {
        self.0.end_occlusion_query();
    }

    fn begin_pipeline_statistics_query(
        &mut self,
        _query_set: &DispatchQuerySet,
        _query_index: u32,
    ) {
        unimplemented!("pipeline statistics queries are not in browser WebGPU")
    }

    fn end_pipeline_statistics_query(&mut self) {
        unimplemented!("pipeline statistics queries are not in browser WebGPU")
    }

    fn execute_bundles(&mut self, render_bundles: &mut dyn Iterator<Item = &DispatchRenderBundle>) {
        let bundles: Vec<&gpu::RenderBundle> = render_bundles
            .map(|bundle| &custom::<RenderBundle>(bundle.as_custom()).0)
            .collect();
        self.0.execute_bundles(&bundles);
    }
}

impl RenderBundleEncoderInterface for RenderBundleEncoder {
    fn set_pipeline(&mut self, pipeline: &DispatchRenderPipeline) {
        self.0
            .set_pipeline(&custom::<RenderPipeline>(pipeline.as_custom()).0);
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: Option<&DispatchBindGroup>,
        offsets: &[wgpu::DynamicOffset],
    ) {
        let group = bind_group.map(|group| &custom::<BindGroup>(group.as_custom()).0);
        self.0.set_bind_group(index, group, offsets);
    }

    fn set_index_buffer(
        &mut self,
        buffer: &DispatchBuffer,
        index_format: wgpu::IndexFormat,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_index_buffer(
            &custom::<Buffer>(buffer.as_custom()).0,
            convert::index_format(index_format),
            offset,
            convert::size(size),
        );
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_vertex_buffer(
            slot,
            &custom::<Buffer>(buffer.as_custom()).0,
            offset,
            convert::size(size),
        );
    }

    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediate data is not in browser WebGPU")
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.0.draw(
            vertices.end.saturating_sub(vertices.start),
            instances.end.saturating_sub(instances.start),
            vertices.start,
            instances.start,
        );
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.0.draw_indexed(
            indices.end.saturating_sub(indices.start),
            instances.end.saturating_sub(instances.start),
            indices.start,
            base_vertex,
            instances.start,
        );
    }

    fn draw_indirect(&mut self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0
            .draw_indirect(&custom::<Buffer>(buffer.as_custom()).0, offset);
    }

    fn draw_indexed_indirect(&mut self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0
            .draw_indexed_indirect(&custom::<Buffer>(buffer.as_custom()).0, offset);
    }

    fn finish(mut self, desc: &wgpu::RenderBundleDescriptor<'_>) -> DispatchRenderBundle {
        self.finish_custom(desc)
    }

    fn finish_custom(&mut self, desc: &wgpu::RenderBundleDescriptor<'_>) -> DispatchRenderBundle {
        let descriptor = abi::RenderBundleDescriptor {
            label: convert::label(desc.label),
        };
        DispatchRenderBundle::custom(RenderBundle::created(self.0.finish(&descriptor)))
    }
}
