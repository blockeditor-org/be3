use core::ops::Range;

use be_wasm_gpu_abi::guest::webgpu as gpu;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use wgpu::custom::*;

use crate::convert::{self, custom, to_web};
use crate::objects::{
    BindGroup, Buffer, CommandBuffer, ComputePipeline, RenderBundle, RenderPipeline, resource,
};

resource!(CommandEncoder, GpuCommandEncoder);
resource!(RenderPass, GpuRenderPassEncoder);
resource!(ComputePass, GpuComputePassEncoder);
resource!(RenderBundleEncoder, GpuRenderBundleEncoder);

fn group(bind_group: Option<&DispatchBindGroup>) -> Option<&gpu::GpuBindGroup> {
    bind_group.map(|group| &custom::<BindGroup>(group.as_custom()).0)
}

fn buffer(buffer: &DispatchBuffer) -> &gpu::GpuBuffer {
    &custom::<Buffer>(buffer.as_custom()).0
}

fn offsets(offsets: &[wgpu::DynamicOffset]) -> Option<&[u32]> {
    (!offsets.is_empty()).then_some(offsets)
}

fn load<V>(load: &wgpu::LoadOp<V>) -> web::GpuLoadOp {
    match load {
        wgpu::LoadOp::Clear(_) => web::GpuLoadOp::Clear,
        wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => web::GpuLoadOp::Load,
    }
}

/// The panic a `set-bind-group` refusal is: wgpu's own backends validate the offsets first.
fn bound(result: Result<(), web::SetBindGroupError>) {
    if let Err(error) = result {
        panic!("{}", error.message);
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        self.0.end();
    }
}

impl Drop for ComputePass {
    fn drop(&mut self) {
        self.0.end();
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
            buffer(source),
            Some(source_offset),
            buffer(destination),
            Some(destination_offset),
            copy_size,
        );
    }

    fn copy_buffer_to_texture(
        &self,
        source: wgpu::TexelCopyBufferInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_buffer_to_texture(
            convert::texel_copy_buffer(source),
            &convert::texel_copy_texture(destination),
            convert::extent(copy_size),
        );
    }

    fn copy_texture_to_buffer(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyBufferInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_texture_to_buffer(
            &convert::texel_copy_texture(source),
            convert::texel_copy_buffer(destination),
            convert::extent(copy_size),
        );
    }

    fn copy_texture_to_texture(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        self.0.copy_texture_to_texture(
            &convert::texel_copy_texture(source),
            &convert::texel_copy_texture(destination),
            convert::extent(copy_size),
        );
    }

    fn begin_compute_pass(&self, desc: &wgpu::ComputePassDescriptor<'_>) -> DispatchComputePass {
        let descriptor = web::GpuComputePassDescriptor {
            timestamp_writes: desc.timestamp_writes.as_ref().map(|writes| {
                web::GpuComputePassTimestampWrites {
                    query_set: convert::query_set_handle(writes.query_set),
                    beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                    end_of_pass_write_index: writes.end_of_pass_write_index,
                }
            }),
            label: convert::label(desc.label),
        };
        DispatchComputePass::custom(ComputePass(self.0.begin_compute_pass(&Some(descriptor))))
    }

    fn begin_render_pass(&self, desc: &wgpu::RenderPassDescriptor<'_>) -> DispatchRenderPass {
        let descriptor = web::GpuRenderPassDescriptor {
            color_attachments: desc
                .color_attachments
                .iter()
                .map(|attachment| {
                    attachment
                        .as_ref()
                        .map(|attachment| web::GpuRenderPassColorAttachment {
                            view: convert::view_handle(attachment.view),
                            depth_slice: attachment.depth_slice,
                            resolve_target: attachment.resolve_target.map(convert::view_handle),
                            clear_value: match attachment.ops.load {
                                wgpu::LoadOp::Clear(value) => Some(convert::color(value)),
                                _ => None,
                            },
                            load_op: load(&attachment.ops.load),
                            store_op: to_web(attachment.ops.store),
                        })
                })
                .collect(),
            depth_stencil_attachment: desc.depth_stencil_attachment.as_ref().map(|attachment| {
                web::GpuRenderPassDepthStencilAttachment {
                    view: convert::view_handle(attachment.view),
                    depth_clear_value: attachment.depth_ops.and_then(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => Some(value),
                        _ => None,
                    }),
                    depth_load_op: attachment.depth_ops.map(|ops| load(&ops.load)),
                    depth_store_op: attachment.depth_ops.map(|ops| to_web(ops.store)),
                    depth_read_only: Some(attachment.depth_ops.is_none()),
                    stencil_clear_value: attachment.stencil_ops.and_then(|ops| match ops.load {
                        wgpu::LoadOp::Clear(value) => Some(value),
                        _ => None,
                    }),
                    stencil_load_op: attachment.stencil_ops.map(|ops| load(&ops.load)),
                    stencil_store_op: attachment.stencil_ops.map(|ops| to_web(ops.store)),
                    stencil_read_only: Some(attachment.stencil_ops.is_none()),
                }
            }),
            occlusion_query_set: desc.occlusion_query_set.map(convert::query_set_handle),
            timestamp_writes: desc.timestamp_writes.as_ref().map(|writes| {
                web::GpuRenderPassTimestampWrites {
                    query_set: convert::query_set_handle(writes.query_set),
                    beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                    end_of_pass_write_index: writes.end_of_pass_write_index,
                }
            }),
            max_draw_count: None,
            label: convert::label(desc.label),
        };
        DispatchRenderPass::custom(RenderPass(self.0.begin_render_pass(&descriptor)))
    }

    fn finish(&mut self) -> DispatchCommandBuffer {
        DispatchCommandBuffer::custom(CommandBuffer(self.0.finish(&None)))
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
        target: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferAddress>,
    ) {
        self.0.clear_buffer(buffer(target), Some(offset), size);
    }

    fn insert_debug_marker(&self, label: &str) {
        self.0.insert_debug_marker(label);
    }

    fn push_debug_group(&self, label: &str) {
        self.0.push_debug_group(label);
    }

    fn pop_debug_group(&self) {
        self.0.pop_debug_group();
    }

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
            &custom::<crate::objects::QuerySet>(query_set.as_custom()).0,
            first_query,
            query_count,
            buffer(destination),
            destination_offset,
        );
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

    /// WebGPU tracks resource states itself.
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
        bound(
            self.0
                .set_bind_group(index, group(bind_group), self::offsets(offsets), None, None),
        );
    }

    fn set_index_buffer(
        &mut self,
        target: &DispatchBuffer,
        index_format: wgpu::IndexFormat,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_index_buffer(
            buffer(target),
            to_web(index_format),
            Some(offset),
            convert::size(size),
        );
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        target: Option<&DispatchBuffer>,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0
            .set_vertex_buffer(slot, target.map(buffer), Some(offset), convert::size(size));
    }

    fn set_immediates(&mut self, offset: u32, data: &[u8]) {
        self.0.set_immediates(offset, data, None, None);
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
            Some(instances.end.saturating_sub(instances.start)),
            Some(vertices.start),
            Some(instances.start),
        );
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.0.draw_indexed(
            indices.end.saturating_sub(indices.start),
            Some(instances.end.saturating_sub(instances.start)),
            Some(indices.start),
            Some(base_vertex),
            Some(instances.start),
        );
    }

    fn draw_mesh_tasks(&mut self, _x: u32, _y: u32, _z: u32) {
        unimplemented!("mesh shaders are not in browser WebGPU")
    }

    fn draw_indirect(&mut self, target: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0.draw_indirect(buffer(target), offset);
    }

    fn draw_indexed_indirect(&mut self, target: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0.draw_indexed_indirect(buffer(target), offset);
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

    fn insert_debug_marker(&mut self, label: &str) {
        self.0.insert_debug_marker(label);
    }

    fn push_debug_group(&mut self, group_label: &str) {
        self.0.push_debug_group(group_label);
    }

    fn pop_debug_group(&mut self) {
        self.0.pop_debug_group();
    }

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
        let bundles: Vec<&gpu::GpuRenderBundle> = render_bundles
            .map(|bundle| &custom::<RenderBundle>(bundle.as_custom()).0)
            .collect();
        self.0.execute_bundles(&bundles);
    }
}

impl ComputePassInterface for ComputePass {
    fn set_pipeline(&mut self, pipeline: &DispatchComputePipeline) {
        self.0
            .set_pipeline(&custom::<ComputePipeline>(pipeline.as_custom()).0);
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: Option<&DispatchBindGroup>,
        offsets: &[wgpu::DynamicOffset],
    ) {
        bound(
            self.0
                .set_bind_group(index, group(bind_group), self::offsets(offsets), None, None),
        );
    }

    fn set_immediates(&mut self, offset: u32, data: &[u8]) {
        self.0.set_immediates(offset, data, None, None);
    }

    /// WebGPU tracks resource states itself, as the command encoder's does.
    fn transition_resources<'a>(
        &mut self,
        _buffer_transitions: &mut dyn Iterator<Item = wgpu::BufferTransition<&'a DispatchBuffer>>,
        _texture_transitions: &mut dyn Iterator<
            Item = wgpu::TextureTransition<&'a DispatchTextureView>,
        >,
    ) {
    }

    fn insert_debug_marker(&mut self, label: &str) {
        self.0.insert_debug_marker(label);
    }

    fn push_debug_group(&mut self, group_label: &str) {
        self.0.push_debug_group(group_label);
    }

    fn pop_debug_group(&mut self) {
        self.0.pop_debug_group();
    }

    fn write_timestamp(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {
        unimplemented!(
            "timestamps written inside passes are not in browser WebGPU: use the pass's timestamp writes"
        )
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

    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.0.dispatch_workgroups(x, Some(y), Some(z));
    }

    fn dispatch_workgroups_indirect(
        &mut self,
        indirect_buffer: &DispatchBuffer,
        indirect_offset: wgpu::BufferAddress,
    ) {
        self.0
            .dispatch_workgroups_indirect(buffer(indirect_buffer), indirect_offset);
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
        bound(
            self.0
                .set_bind_group(index, group(bind_group), self::offsets(offsets), None, None),
        );
    }

    fn set_index_buffer(
        &mut self,
        target: &DispatchBuffer,
        index_format: wgpu::IndexFormat,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0.set_index_buffer(
            buffer(target),
            to_web(index_format),
            Some(offset),
            convert::size(size),
        );
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        target: Option<&DispatchBuffer>,
        offset: wgpu::BufferAddress,
        size: Option<wgpu::BufferSize>,
    ) {
        self.0
            .set_vertex_buffer(slot, target.map(buffer), Some(offset), convert::size(size));
    }

    fn set_immediates(&mut self, offset: u32, data: &[u8]) {
        self.0.set_immediates(offset, data, None, None);
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.0.draw(
            vertices.end.saturating_sub(vertices.start),
            Some(instances.end.saturating_sub(instances.start)),
            Some(vertices.start),
            Some(instances.start),
        );
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.0.draw_indexed(
            indices.end.saturating_sub(indices.start),
            Some(instances.end.saturating_sub(instances.start)),
            Some(indices.start),
            Some(base_vertex),
            Some(instances.start),
        );
    }

    fn draw_indirect(&mut self, target: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0.draw_indirect(buffer(target), offset);
    }

    fn draw_indexed_indirect(&mut self, target: &DispatchBuffer, offset: wgpu::BufferAddress) {
        self.0.draw_indexed_indirect(buffer(target), offset);
    }

    fn finish(self, desc: &wgpu::RenderBundleDescriptor<'_>) -> DispatchRenderBundle {
        Box::new(self).finish_boxed(desc)
    }

    fn finish_boxed(
        self: Box<Self>,
        desc: &wgpu::RenderBundleDescriptor<'_>,
    ) -> DispatchRenderBundle {
        let descriptor = web::GpuRenderBundleDescriptor {
            label: convert::label(desc.label),
        };
        DispatchRenderBundle::custom(RenderBundle(self.0.finish(&Some(descriptor))))
    }
}
