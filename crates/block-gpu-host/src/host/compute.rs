use block_gpu_abi::device as abi;

use crate::{Gpu, Outcome, convert};

impl Gpu {
    pub(super) fn try_create_compute_pipeline(
        &mut self,
        descriptor: &abi::ComputePipelineDescriptor,
    ) -> Outcome<wgpu::ComputePipeline> {
        let layout = descriptor
            .layout
            .map(|handle| {
                self.pipeline_layouts
                    .get(handle.0, "pipeline layout")
                    .cloned()
            })
            .transpose()?;
        let module = self
            .modules
            .get(descriptor.module.0, "shader module")?
            .clone();
        Ok(self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: convert::label(&descriptor.label),
                layout: layout.as_ref(),
                module: &module,
                entry_point: descriptor.entry_point.as_deref(),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            }))
    }

    pub(super) fn try_begin_compute_pass(
        &mut self,
        encoder: abi::CommandEncoder,
        descriptor: &abi::ComputePassDescriptor,
    ) -> Outcome<wgpu::ComputePass<'static>> {
        let encoder = self.encoders.get_mut(encoder.0, "command encoder")?;
        Ok(encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: convert::label(&descriptor.label),
                timestamp_writes: None,
            })
            .forget_lifetime())
    }

    pub(super) fn compute_pass(
        &mut self,
        pass: abi::ComputePass,
        act: impl FnOnce(&mut wgpu::ComputePass<'static>),
    ) {
        let result = self.compute_passes.get_mut(pass.0, "compute pass").map(act);
        self.fail(result, ());
    }
}
