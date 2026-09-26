use std::collections::HashMap;

use block_gpu_abi as abi;
use serde::{Deserialize, Serialize};

use crate::{Gpu, SURFACE_USAGE, tables::Counter};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Call {
    CreateBuffer(Vec<u8>),
    CreateTexture(Vec<u8>),
    CreateTextureView(Vec<u8>),
    CreateSampler(Vec<u8>),
    CreateBindGroupLayout(Vec<u8>),
    CreateBindGroup(Vec<u8>),
    CreatePipelineLayout(Vec<u8>),
    CreateShaderModule(Vec<u8>),
    CreateRenderPipeline(Vec<u8>),
    CreateCommandEncoder(Vec<u8>),
    WriteMappedBuffer {
        buffer: abi::Handle,
        offset: u64,
        data: Vec<u8>,
    },
    UnmapBuffer(abi::Handle),
    WriteBuffer {
        buffer: abi::Handle,
        offset: u64,
        data: Vec<u8>,
    },
    WriteTexture {
        request: Vec<u8>,
        data: Vec<u8>,
    },
    CopyTextureToTexture(Vec<u8>),
    Submit(Vec<abi::Handle>),
    BeginRenderPass(Vec<u8>),
    FinishEncoder(abi::Handle),
    SetPipeline {
        pass: abi::Handle,
        pipeline: abi::Handle,
    },
    SetBindGroup {
        pass: abi::Handle,
        index: u32,
        group: abi::Handle,
        offsets: Vec<u32>,
    },
    SetIndexBuffer {
        pass: abi::Handle,
        buffer: abi::Handle,
        format: u32,
        offset: u64,
        size: u64,
    },
    SetVertexBuffer {
        pass: abi::Handle,
        slot: u32,
        buffer: abi::Handle,
        offset: u64,
        size: u64,
    },
    SetViewport {
        pass: abi::Handle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    },
    SetScissorRect {
        pass: abi::Handle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    SetBlendConstant {
        pass: abi::Handle,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    },
    SetStencilReference {
        pass: abi::Handle,
        reference: u32,
    },
    Draw {
        pass: abi::Handle,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    },
    DrawIndexed {
        pass: abi::Handle,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    },
    EndPass(abi::Handle),
    DropResource {
        kind: u32,
        handle: abi::Handle,
    },
    ConfigureSurface {
        surface: u32,
        configuration: Vec<u8>,
    },
    AcquireSurface(u32),
    PresentSurface(u32),
}

impl Gpu {
    pub fn apply(&mut self, call: Call) {
        match call {
            Call::CreateBuffer(bytes) => {
                self.create_buffer(&bytes);
            }
            Call::CreateTexture(bytes) => {
                self.create_texture(&bytes);
            }
            Call::CreateTextureView(bytes) => {
                self.create_texture_view(&bytes);
            }
            Call::CreateSampler(bytes) => {
                self.create_sampler(&bytes);
            }
            Call::CreateBindGroupLayout(bytes) => {
                self.create_bind_group_layout(&bytes);
            }
            Call::CreateBindGroup(bytes) => {
                self.create_bind_group(&bytes);
            }
            Call::CreatePipelineLayout(bytes) => {
                self.create_pipeline_layout(&bytes);
            }
            Call::CreateShaderModule(bytes) => {
                self.create_shader_module(&bytes);
            }
            Call::CreateRenderPipeline(bytes) => {
                self.create_render_pipeline(&bytes);
            }
            Call::CreateCommandEncoder(bytes) => {
                self.create_command_encoder(&bytes);
            }
            Call::WriteMappedBuffer {
                buffer,
                offset,
                data,
            } => self.write_mapped_buffer(buffer, offset, &data),
            Call::UnmapBuffer(buffer) => self.unmap_buffer(buffer),
            Call::WriteBuffer {
                buffer,
                offset,
                data,
            } => self.write_buffer(buffer, offset, &data),
            Call::WriteTexture { request, data } => self.write_texture(&request, &data),
            Call::CopyTextureToTexture(bytes) => self.copy_texture_to_texture(&bytes),
            Call::Submit(handles) => self.submit(&handles),
            Call::BeginRenderPass(bytes) => {
                self.begin_render_pass(&bytes);
            }
            Call::FinishEncoder(encoder) => {
                self.finish_encoder(encoder);
            }
            Call::SetPipeline { pass, pipeline } => self.set_pipeline(pass, pipeline),
            Call::SetBindGroup {
                pass,
                index,
                group,
                offsets,
            } => self.set_bind_group(pass, index, group, &offsets),
            Call::SetIndexBuffer {
                pass,
                buffer,
                format,
                offset,
                size,
            } => self.set_index_buffer(pass, buffer, format, offset, size),
            Call::SetVertexBuffer {
                pass,
                slot,
                buffer,
                offset,
                size,
            } => self.set_vertex_buffer(pass, slot, buffer, offset, size),
            Call::SetViewport {
                pass,
                x,
                y,
                width,
                height,
                minimum_depth,
                maximum_depth,
            } => self.set_viewport(pass, x, y, width, height, minimum_depth, maximum_depth),
            Call::SetScissorRect {
                pass,
                x,
                y,
                width,
                height,
            } => self.set_scissor_rect(pass, x, y, width, height),
            Call::SetBlendConstant {
                pass,
                red,
                green,
                blue,
                alpha,
            } => self.set_blend_constant(pass, red, green, blue, alpha),
            Call::SetStencilReference { pass, reference } => {
                self.set_stencil_reference(pass, reference)
            }
            Call::Draw {
                pass,
                first_vertex,
                vertex_count,
                first_instance,
                instance_count,
            } => self.draw(
                pass,
                first_vertex,
                vertex_count,
                first_instance,
                instance_count,
            ),
            Call::DrawIndexed {
                pass,
                first_index,
                index_count,
                base_vertex,
                first_instance,
                instance_count,
            } => self.draw_indexed(
                pass,
                first_index,
                index_count,
                base_vertex,
                first_instance,
                instance_count,
            ),
            Call::EndPass(pass) => self.end_pass(pass),
            Call::DropResource { kind, handle } => self.drop_resource(kind, handle),
            Call::ConfigureSurface {
                surface,
                configuration,
            } => self.configure_surface(surface, &configuration),
            Call::AcquireSurface(surface) => {
                self.acquire_surface(surface);
            }
            Call::PresentSurface(surface) => self.present_surface(surface),
        }
    }
}

struct Counters {
    buffers: Counter,
    textures: Counter,
    views: Counter,
    samplers: Counter,
    group_layouts: Counter,
    groups: Counter,
    pipeline_layouts: Counter,
    modules: Counter,
    pipelines: Counter,
    encoders: Counter,
    command_buffers: Counter,
    passes: Counter,
}

pub struct Recorder {
    limits: abi::DeviceLimits,
    counters: Counters,
    textures: HashMap<abi::Handle, abi::TextureDescriptor>,
    surfaces: HashMap<u32, abi::TextureDescriptor>,
    calls: Vec<Call>,
    error: Option<String>,
}

impl Recorder {
    pub fn new(limits: &[u8]) -> Result<Self, String> {
        Ok(Self {
            limits: abi::decode(limits)?,
            counters: Counters {
                buffers: Counter::new(),
                textures: Counter::new(),
                views: Counter::new(),
                samplers: Counter::new(),
                group_layouts: Counter::new(),
                groups: Counter::new(),
                pipeline_layouts: Counter::new(),
                modules: Counter::new(),
                pipelines: Counter::new(),
                encoders: Counter::new(),
                command_buffers: Counter::new(),
                passes: Counter::new(),
            },
            textures: HashMap::new(),
            surfaces: HashMap::new(),
            calls: Vec::new(),
            error: None,
        })
    }

    pub fn take_calls(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.calls)
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    pub fn report(&mut self, message: String) {
        if self.error.is_none() {
            self.error = Some(message);
        }
    }

    pub fn limits(&self) -> Vec<u8> {
        abi::encode(&self.limits)
    }

    pub fn create_buffer(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateBuffer(bytes.to_vec()));
        self.counters.buffers.take()
    }

    pub fn create_texture(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateTexture(bytes.to_vec()));
        let handle = self.counters.textures.take();
        match abi::decode::<abi::TextureDescriptor>(bytes) {
            Ok(descriptor) => {
                self.textures.insert(
                    handle,
                    abi::TextureDescriptor {
                        label: String::new(),
                        view_formats: Vec::new(),
                        ..descriptor
                    },
                );
            }
            Err(message) => self.report(message),
        }
        handle
    }

    pub fn create_texture_view(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateTextureView(bytes.to_vec()));
        self.counters.views.take()
    }

    pub fn create_sampler(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateSampler(bytes.to_vec()));
        self.counters.samplers.take()
    }

    pub fn create_bind_group_layout(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateBindGroupLayout(bytes.to_vec()));
        self.counters.group_layouts.take()
    }

    pub fn create_bind_group(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateBindGroup(bytes.to_vec()));
        self.counters.groups.take()
    }

    pub fn create_pipeline_layout(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreatePipelineLayout(bytes.to_vec()));
        self.counters.pipeline_layouts.take()
    }

    pub fn create_shader_module(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateShaderModule(bytes.to_vec()));
        self.counters.modules.take()
    }

    pub fn create_render_pipeline(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateRenderPipeline(bytes.to_vec()));
        self.counters.pipelines.take()
    }

    pub fn create_command_encoder(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::CreateCommandEncoder(bytes.to_vec()));
        self.counters.encoders.take()
    }

    pub fn write_mapped_buffer(&mut self, buffer: abi::Handle, offset: u64, data: &[u8]) {
        self.calls.push(Call::WriteMappedBuffer {
            buffer,
            offset,
            data: data.to_vec(),
        });
    }

    pub fn unmap_buffer(&mut self, buffer: abi::Handle) {
        self.calls.push(Call::UnmapBuffer(buffer));
    }

    pub fn write_buffer(&mut self, buffer: abi::Handle, offset: u64, data: &[u8]) {
        self.calls.push(Call::WriteBuffer {
            buffer,
            offset,
            data: data.to_vec(),
        });
    }

    pub fn write_texture(&mut self, bytes: &[u8], data: &[u8]) {
        self.calls.push(Call::WriteTexture {
            request: bytes.to_vec(),
            data: data.to_vec(),
        });
    }

    pub fn copy_texture_to_texture(&mut self, bytes: &[u8]) {
        self.calls.push(Call::CopyTextureToTexture(bytes.to_vec()));
    }

    pub fn submit(&mut self, handles: &[u32]) {
        self.calls.push(Call::Submit(handles.to_vec()));
    }

    pub fn begin_render_pass(&mut self, bytes: &[u8]) -> abi::Handle {
        self.calls.push(Call::BeginRenderPass(bytes.to_vec()));
        self.counters.passes.take()
    }

    pub fn finish_encoder(&mut self, encoder: abi::Handle) -> abi::Handle {
        self.calls.push(Call::FinishEncoder(encoder));
        self.counters.command_buffers.take()
    }

    pub fn set_pipeline(&mut self, pass: abi::Handle, pipeline: abi::Handle) {
        self.calls.push(Call::SetPipeline { pass, pipeline });
    }

    pub fn set_bind_group(
        &mut self,
        pass: abi::Handle,
        index: u32,
        group: abi::Handle,
        offsets: &[u32],
    ) {
        self.calls.push(Call::SetBindGroup {
            pass,
            index,
            group,
            offsets: offsets.to_vec(),
        });
    }

    pub fn set_index_buffer(
        &mut self,
        pass: abi::Handle,
        buffer: abi::Handle,
        format: u32,
        offset: u64,
        size: u64,
    ) {
        self.calls.push(Call::SetIndexBuffer {
            pass,
            buffer,
            format,
            offset,
            size,
        });
    }

    pub fn set_vertex_buffer(
        &mut self,
        pass: abi::Handle,
        slot: u32,
        buffer: abi::Handle,
        offset: u64,
        size: u64,
    ) {
        self.calls.push(Call::SetVertexBuffer {
            pass,
            slot,
            buffer,
            offset,
            size,
        });
    }

    pub fn set_viewport(
        &mut self,
        pass: abi::Handle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    ) {
        self.calls.push(Call::SetViewport {
            pass,
            x,
            y,
            width,
            height,
            minimum_depth,
            maximum_depth,
        });
    }

    pub fn set_scissor_rect(&mut self, pass: abi::Handle, x: u32, y: u32, width: u32, height: u32) {
        self.calls.push(Call::SetScissorRect {
            pass,
            x,
            y,
            width,
            height,
        });
    }

    pub fn set_blend_constant(
        &mut self,
        pass: abi::Handle,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    ) {
        self.calls.push(Call::SetBlendConstant {
            pass,
            red,
            green,
            blue,
            alpha,
        });
    }

    pub fn set_stencil_reference(&mut self, pass: abi::Handle, reference: u32) {
        self.calls
            .push(Call::SetStencilReference { pass, reference });
    }

    pub fn draw(
        &mut self,
        pass: abi::Handle,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    ) {
        self.calls.push(Call::Draw {
            pass,
            first_vertex,
            vertex_count,
            first_instance,
            instance_count,
        });
    }

    pub fn draw_indexed(
        &mut self,
        pass: abi::Handle,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    ) {
        self.calls.push(Call::DrawIndexed {
            pass,
            first_index,
            index_count,
            base_vertex,
            first_instance,
            instance_count,
        });
    }

    pub fn end_pass(&mut self, pass: abi::Handle) {
        self.calls.push(Call::EndPass(pass));
    }

    pub fn drop_resource(&mut self, kind: u32, handle: abi::Handle) {
        if abi::ResourceKind::from_code(kind) == Some(abi::ResourceKind::Texture) {
            self.textures.remove(&handle);
        }
        self.calls.push(Call::DropResource { kind, handle });
    }

    pub fn configure_surface(&mut self, surface: u32, bytes: &[u8]) {
        self.calls.push(Call::ConfigureSurface {
            surface,
            configuration: bytes.to_vec(),
        });
        let configuration: abi::SurfaceConfiguration = match abi::decode(bytes) {
            Ok(configuration) => configuration,
            Err(message) => return self.report(message),
        };
        let limit = self.limits.max_texture_dimension_2d;
        let abi::SurfaceConfiguration {
            width,
            height,
            format,
        } = configuration;
        if width == 0 || height == 0 || width > limit || height > limit {
            return self.report(format!(
                "surface {surface} asked for a {width} by {height} target, which this device cannot make"
            ));
        }
        self.surfaces.insert(
            surface,
            abi::TextureDescriptor {
                label: String::new(),
                size: abi::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: abi::TextureDimension::D2,
                format,
                usage: SURFACE_USAGE.bits(),
                view_formats: Vec::new(),
            },
        );
    }

    pub fn acquire_surface(&mut self, surface: u32) -> abi::Handle {
        self.calls.push(Call::AcquireSurface(surface));
        let handle = self.counters.textures.take();
        let Some(descriptor) = self.surfaces.get(&surface).cloned() else {
            self.report(format!("surface {surface} has no target texture"));
            return abi::NULL_HANDLE;
        };
        self.textures.insert(handle, descriptor);
        handle
    }

    pub fn present_surface(&mut self, surface: u32) {
        self.calls.push(Call::PresentSurface(surface));
    }

    pub fn describe_texture(&mut self, texture: abi::Handle) -> Option<Vec<u8>> {
        match self.textures.get(&texture) {
            Some(descriptor) => Some(abi::encode(descriptor)),
            None => {
                self.report(format!("no texture is registered as handle {texture}"));
                None
            }
        }
    }
}
