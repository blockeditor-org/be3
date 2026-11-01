use std::ops::Range;

use block_gpu_abi::device as abi;

use crate::{Outcome, convert, slice};

const MAX_COLOR_ATTACHMENTS: usize = 8;

pub(crate) struct BundleEncoder {
    label: Option<String>,
    color_formats: Vec<Option<wgpu::TextureFormat>>,
    depth_stencil: Option<wgpu::RenderBundleDepthStencil>,
    sample_count: u32,
    commands: Vec<Command>,
}

pub(crate) enum Command {
    Pipeline(wgpu::RenderPipeline),
    BindGroup(u32, Option<wgpu::BindGroup>, Vec<u32>),
    IndexBuffer(wgpu::Buffer, wgpu::IndexFormat, u64, Option<u64>),
    VertexBuffer(u32, wgpu::Buffer, u64, Option<u64>),
    Draw(Range<u32>, Range<u32>),
    DrawIndexed(Range<u32>, i32, Range<u32>),
    DrawIndirect(wgpu::Buffer, u64),
    DrawIndexedIndirect(wgpu::Buffer, u64),
}

impl BundleEncoder {
    pub(crate) fn new(descriptor: abi::RenderBundleEncoderDescriptor) -> Outcome<Self> {
        let colors = descriptor.color_formats.len();
        if colors > MAX_COLOR_ATTACHMENTS {
            return Err(format!(
                "a render bundle encoder has {colors} color formats, more than {}",
                MAX_COLOR_ATTACHMENTS
            ));
        }
        let count = descriptor.sample_count;
        if count == 0 || count > 32 || !count.is_power_of_two() {
            return Err(format!(
                "a render bundle encoder cannot have a sample count of {count}"
            ));
        }
        Ok(Self {
            label: descriptor.label,
            color_formats: descriptor
                .color_formats
                .iter()
                .map(|format| format.map(wgpu::TextureFormat::from))
                .collect(),
            depth_stencil: descriptor
                .depth_stencil
                .map(|state| wgpu::RenderBundleDepthStencil {
                    format: wgpu::TextureFormat::from(state.format),
                    depth_read_only: state.depth_read_only,
                    stencil_read_only: state.stencil_read_only,
                }),
            sample_count: count,
            commands: Vec::new(),
        })
    }

    pub(crate) fn push(&mut self, command: Command) -> Outcome<()> {
        if let Command::IndexBuffer(buffer, _, offset, size)
        | Command::VertexBuffer(_, buffer, offset, size) = &command
        {
            slice(buffer, *offset, *size)?;
        }
        self.commands.push(command);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        device: &wgpu::Device,
        descriptor: &abi::RenderBundleDescriptor,
    ) -> Outcome<wgpu::RenderBundle> {
        let mut encoder =
            device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: convert::label(&self.label),
                color_formats: &self.color_formats,
                depth_stencil: self.depth_stencil,
                sample_count: self.sample_count,
                multiview: None,
            });
        for command in &self.commands {
            match command {
                Command::Pipeline(pipeline) => encoder.set_pipeline(pipeline),
                Command::BindGroup(index, group, offsets) => {
                    encoder.set_bind_group(*index, group.as_ref(), offsets)
                }
                Command::IndexBuffer(buffer, format, offset, size) => {
                    encoder.set_index_buffer(slice(buffer, *offset, *size)?, *format)
                }
                Command::VertexBuffer(slot, buffer, offset, size) => {
                    encoder.set_vertex_buffer(*slot, slice(buffer, *offset, *size)?)
                }
                Command::Draw(vertices, instances) => {
                    encoder.draw(vertices.clone(), instances.clone())
                }
                Command::DrawIndexed(indices, base, instances) => {
                    encoder.draw_indexed(indices.clone(), *base, instances.clone())
                }
                Command::DrawIndirect(buffer, offset) => encoder.draw_indirect(buffer, *offset),
                Command::DrawIndexedIndirect(buffer, offset) => {
                    encoder.draw_indexed_indirect(buffer, *offset)
                }
            }
        }
        Ok(encoder.finish(&wgpu::RenderBundleDescriptor {
            label: convert::label(&descriptor.label),
        }))
    }
}

impl crate::Gpu {
    pub(crate) fn bundle(
        &mut self,
        this: abi::RenderBundleEncoder,
        command: impl FnOnce(&Self) -> Outcome<Command>,
    ) {
        let result = command(self).and_then(|command| {
            self.bundle_encoders
                .get_mut(this.0, "render bundle encoder")?
                .push(command)
        });
        self.fail(result, ());
    }
}
