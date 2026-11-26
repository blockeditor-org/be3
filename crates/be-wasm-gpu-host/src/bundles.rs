use std::ops::Range;

use crate::{Outcome, slice, vertex_slice};

const MAX_COLOR_ATTACHMENTS: usize = 8;

/// A render bundle the host records itself and replays into wgpu's encoder at `finish`,
/// since wgpu's bundle encoder borrows what it is given. wgpu has no debug markers on
/// bundle encoders: wasi:webgpu's debug groups are only checked to balance.
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
    VertexBuffer(u32, Option<wgpu::Buffer>, u64, Option<u64>),
    Draw(Range<u32>, Range<u32>),
    DrawIndexed(Range<u32>, i32, Range<u32>),
    DrawIndirect(wgpu::Buffer, u64),
    DrawIndexedIndirect(wgpu::Buffer, u64),
    Immediates(u32, Vec<u8>),
    PushDebugGroup,
    PopDebugGroup,
}

impl BundleEncoder {
    pub(crate) fn with(
        label: Option<String>,
        color_formats: Vec<Option<wgpu::TextureFormat>>,
        depth_stencil: Option<wgpu::RenderBundleDepthStencil>,
        count: u32,
    ) -> Outcome<Self> {
        let colors = color_formats.len();
        if colors > MAX_COLOR_ATTACHMENTS {
            return Err(format!(
                "a render bundle encoder has {colors} color formats, more than {}",
                MAX_COLOR_ATTACHMENTS
            ));
        }
        if count == 0 || count > 32 || !count.is_power_of_two() {
            return Err(format!(
                "a render bundle encoder cannot have a sample count of {count}"
            ));
        }
        Ok(Self {
            label,
            color_formats,
            depth_stencil,
            sample_count: count,
            commands: Vec::new(),
        })
    }

    pub(crate) fn push(&mut self, command: Command) -> Outcome<()> {
        match &command {
            Command::IndexBuffer(buffer, _, offset, size) => {
                slice(buffer, *offset, *size)?;
            }
            Command::VertexBuffer(slot, buffer, offset, size) => {
                vertex_slice(buffer.as_ref(), *slot, *offset, *size)?;
            }
            _ => {}
        }
        self.commands.push(command);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        device: &wgpu::Device,
        label: Option<&str>,
    ) -> Outcome<wgpu::RenderBundle> {
        let mut depth = 0usize;
        for command in &self.commands {
            match command {
                Command::PushDebugGroup => depth += 1,
                Command::PopDebugGroup => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or("a render bundle pops a debug group it never pushed")?;
                }
                _ => {}
            }
        }
        if depth > 0 {
            return Err(format!("a render bundle leaves {depth} debug groups open"));
        }
        let mut encoder =
            device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: self.label.as_deref(),
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
                Command::VertexBuffer(slot, buffer, offset, size) => encoder.set_vertex_buffer(
                    *slot,
                    vertex_slice(buffer.as_ref(), *slot, *offset, *size)?,
                ),
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
                Command::Immediates(offset, data) => encoder.set_immediates(*offset, data),
                Command::PushDebugGroup | Command::PopDebugGroup => {}
            }
        }
        Ok(encoder.finish(&wgpu::RenderBundleDescriptor { label }))
    }
}
