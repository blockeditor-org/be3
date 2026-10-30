use block_gpu_abi::{device as abi, guest::device as gpu};
use wgpu::custom::*;

use super::{BindGroup, BindGroupLayout, Buffer, QuerySet, checked, last_error, reject, resource};
use crate::convert::{self, custom};

resource!(ComputePipeline);
resource!(ComputePass);

pub(super) fn pipeline(desc: &wgpu::ComputePipelineDescriptor<'_>) -> DispatchComputePipeline {
    let descriptor = abi::ComputePipelineDescriptor {
        label: convert::label(desc.label),
        layout: desc.layout.map(convert::layout_handle),
        module: convert::module_handle(desc.module),
        entry_point: desc.entry_point.map(str::to_owned),
        constants: convert::constants(&desc.compilation_options),
    };
    DispatchComputePipeline::custom(ComputePipeline::created(gpu::create_compute_pipeline(
        &descriptor,
    )))
}

pub(super) fn pass(
    encoder: &gpu::CommandEncoder,
    desc: &wgpu::ComputePassDescriptor<'_>,
) -> DispatchComputePass {
    let descriptor = abi::ComputePassDescriptor {
        label: convert::label(desc.label),
        timestamp_writes: desc
            .timestamp_writes
            .as_ref()
            .map(|writes| abi::PassTimestampWrites {
                query_set: custom::<QuerySet>(writes.query_set.as_custom()).0.handle(),
                beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                end_of_pass_write_index: writes.end_of_pass_write_index,
            }),
    };
    DispatchComputePass::custom(ComputePass::created(
        encoder.begin_compute_pass(&descriptor),
    ))
}

impl ComputePipelineInterface for ComputePipeline {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        DispatchBindGroupLayout::custom(BindGroupLayout::created(
            self.0.get_bind_group_layout(index),
        ))
    }
}

impl Drop for ComputePass {
    fn drop(&mut self) {
        self.0.end();
        checked();
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
        let group = bind_group.map(|group| &custom::<BindGroup>(group.as_custom()).0);
        self.0.set_bind_group(index, group, offsets);
    }

    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediate data is not in browser WebGPU")
    }

    fn insert_debug_marker(&mut self, _label: &str) {}

    fn push_debug_group(&mut self, _group_label: &str) {}

    fn pop_debug_group(&mut self) {}

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
        self.0.dispatch_workgroups(x, y, z);
    }

    fn dispatch_workgroups_indirect(
        &mut self,
        indirect_buffer: &DispatchBuffer,
        indirect_offset: wgpu::BufferAddress,
    ) {
        self.0.dispatch_workgroups_indirect(
            &custom::<Buffer>(indirect_buffer.as_custom()).0,
            indirect_offset,
        );
    }
}
