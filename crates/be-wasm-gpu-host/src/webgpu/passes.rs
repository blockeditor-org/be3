use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::Convert;

use super::objects::found;
use super::{Failure, Kind};
use crate::Gpu;
use crate::bundles::{BundleEncoder, Command};
use crate::queries::Coarsening;
use crate::tables::Table;

type Recorded = Box<dyn FnOnce(&mut wgpu::CommandEncoder)>;

/// A pass being recorded, with the app device it records for, borrowed in place by each call.
pub(crate) struct Recording<P> {
    pass: std::cell::RefCell<P>,
    device: Option<u32>,
}

impl<P> Recording<P> {
    fn new(pass: P, device: Option<u32>) -> Self {
        Self {
            pass: std::cell::RefCell::new(pass),
            device,
        }
    }
}

impl Gpu {
    /// Records into an encoder. wgpu keeps what a command got wrong in the encoder and raises
    /// it at `finish`, so only a missing object is raised here.
    pub(super) fn record(
        &mut self,
        encoder: u32,
        act: impl FnOnce(&mut Self) -> Result<Recorded, Failure>,
    ) {
        let device = self.owner(Kind::Encoder, encoder);
        self.checked(device, |gpu| {
            let record = act(gpu)?;
            record(found(gpu.encoders.get_mut(encoder, "command encoder"))?);
            Ok(())
        });
    }

    /// A call made per draw on a render pass: not wrapped in scopes, since wgpu raises a pass's
    /// errors when it ends. It looks up only the pass, which keeps its device, and records in
    /// place while `act` reads the other tables.
    pub(super) fn render(
        &mut self,
        pass: u32,
        act: impl FnOnce(&Self, &mut wgpu::RenderPass<'static>) -> Result<(), Failure>,
    ) {
        self.recorded(
            |gpu| &gpu.passes,
            Kind::RenderPass,
            pass,
            "render pass",
            act,
        );
    }

    /// A call made per dispatch on a compute pass, as `render`.
    pub(super) fn compute(
        &mut self,
        pass: u32,
        act: impl FnOnce(&Self, &mut wgpu::ComputePass<'static>) -> Result<(), Failure>,
    ) {
        self.recorded(
            |gpu| &gpu.compute_passes,
            Kind::ComputePass,
            pass,
            "compute pass",
            act,
        );
    }

    fn recorded<P>(
        &mut self,
        table: fn(&Self) -> &Table<Recording<P>>,
        kind: Kind,
        pass: u32,
        name: &str,
        act: impl FnOnce(&Self, &mut P) -> Result<(), Failure>,
    ) {
        let device = match table(self).get(pass, name) {
            Ok(recording) => recording.device,
            // A pass ended is still its device's until it is dropped, whose scopes hear why.
            Err(message) => {
                let device = self.owner(kind, pass);
                if !device.is_some_and(|device| self.device_lost(device)) {
                    self.raise(device, Failure::validation(message));
                }
                return;
            }
        };
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        let outer = std::mem::replace(&mut self.web.acting, device);
        let result = match table(self).get(pass, name) {
            Ok(recording) => act(self, &mut recording.pass.borrow_mut()),
            Err(message) => Err(Failure::validation(message)),
        };
        self.web.acting = outer;
        if let Err(failure) = result {
            self.raise(device, failure);
        }
    }

    /// Records a command into a render bundle encoder, which replays them all at `finish`.
    pub(super) fn bundled(
        &mut self,
        encoder: u32,
        command: impl FnOnce(&Self) -> Result<Command, Failure>,
    ) {
        let device = self.owner(Kind::BundleEncoder, encoder);
        self.checked(device, |gpu| {
            let command = command(gpu)?;
            found(
                found(
                    gpu.bundle_encoders
                        .get_mut(encoder, "render bundle encoder"),
                )?
                .push(command),
            )
        });
    }

    pub(super) fn bundle_encoder(
        &mut self,
        device: u32,
        descriptor: web::GpuRenderBundleEncoderDescriptor,
    ) -> u32 {
        let label = descriptor.label.clone();
        self.make(
            Some(device),
            Kind::BundleEncoder,
            label.as_deref(),
            |gpu| &mut gpu.bundle_encoders,
            |_| {
                let depth_stencil =
                    descriptor
                        .depth_stencil_format
                        .map(|format| wgpu::RenderBundleDepthStencil {
                            format: format.convert(),
                            depth_read_only: descriptor.depth_read_only.unwrap_or(false),
                            stencil_read_only: descriptor.stencil_read_only.unwrap_or(false),
                        });
                found(BundleEncoder::with(
                    descriptor.label.clone(),
                    descriptor
                        .color_formats
                        .iter()
                        .map(|format| format.map(Convert::convert))
                        .collect(),
                    depth_stencil,
                    descriptor.sample_count.unwrap_or(1),
                ))
            },
        )
    }

    pub(super) fn begin_render_pass(
        &mut self,
        encoder: u32,
        descriptor: web::GpuRenderPassDescriptor,
    ) -> u32 {
        let device = self.owner(Kind::Encoder, encoder);
        let label = descriptor.label.clone();
        self.make(
            device,
            Kind::RenderPass,
            label.as_deref(),
            |gpu| &mut gpu.passes,
            |gpu| {
                let mut colors = Vec::new();
                for attachment in &descriptor.color_attachments {
                    let Some(attachment) = attachment else {
                        colors.push(None);
                        continue;
                    };
                    let view = gpu
                        .held(&gpu.views, Kind::View, attachment.view.0, "texture view")?
                        .clone();
                    let resolve = attachment
                        .resolve_target
                        .map(|view| {
                            gpu.held(&gpu.views, Kind::View, view.0, "texture view")
                                .cloned()
                        })
                        .transpose()?;
                    colors.push(Some((attachment, view, resolve)));
                }
                let depth = descriptor
                    .depth_stencil_attachment
                    .as_ref()
                    .map(|attachment| {
                        gpu.held(&gpu.views, Kind::View, attachment.view.0, "texture view")
                            .map(|view| (attachment, view.clone()))
                    })
                    .transpose()?;
                let occlusion = descriptor
                    .occlusion_query_set
                    .map(|set| {
                        gpu.held(&gpu.query_sets, Kind::QuerySet, set.0, "query set")
                            .map(|set| set.set.clone())
                    })
                    .transpose()?;
                let timestamps = descriptor
                    .timestamp_writes
                    .map(|writes| {
                        gpu.held(
                            &gpu.query_sets,
                            Kind::QuerySet,
                            writes.query_set.0,
                            "query set",
                        )
                        .map(|set| (set.set.clone(), writes))
                    })
                    .transpose()?;
                let color_attachments: Vec<_> = colors
                    .iter()
                    .map(|attachment| {
                        attachment.as_ref().map(|(attachment, view, resolve)| {
                            wgpu::RenderPassColorAttachment {
                                view,
                                depth_slice: attachment.depth_slice,
                                resolve_target: resolve.as_ref(),
                                ops: wgpu::Operations {
                                    load: match attachment.load_op {
                                        web::GpuLoadOp::Clear => wgpu::LoadOp::Clear(
                                            attachment
                                                .clear_value
                                                .map_or(wgpu::Color::TRANSPARENT, Convert::convert),
                                        ),
                                        web::GpuLoadOp::Load => wgpu::LoadOp::Load,
                                    },
                                    store: attachment.store_op.convert(),
                                },
                            }
                        })
                    })
                    .collect();
                let depth_stencil_attachment = depth.as_ref().map(|(attachment, view)| {
                    let ops = |load: Option<web::GpuLoadOp>,
                               store: Option<web::GpuStoreOp>,
                               read_only: Option<bool>,
                               clear| {
                        match (read_only.unwrap_or(false), load) {
                            (true, _) | (false, None) => None,
                            (false, Some(load)) => Some(wgpu::Operations {
                                load: match load {
                                    web::GpuLoadOp::Clear => wgpu::LoadOp::Clear(clear),
                                    web::GpuLoadOp::Load => wgpu::LoadOp::Load,
                                },
                                store: store.map_or(wgpu::StoreOp::Store, Convert::convert),
                            }),
                        }
                    };
                    wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: ops(
                            attachment.depth_load_op,
                            attachment.depth_store_op,
                            attachment.depth_read_only,
                            attachment.depth_clear_value.unwrap_or(0.0),
                        ),
                        stencil_ops: match (
                            attachment.stencil_read_only.unwrap_or(false),
                            attachment.stencil_load_op,
                        ) {
                            (true, _) | (false, None) => None,
                            (false, Some(load)) => Some(wgpu::Operations {
                                load: match load {
                                    web::GpuLoadOp::Clear => wgpu::LoadOp::Clear(
                                        attachment.stencil_clear_value.unwrap_or(0),
                                    ),
                                    web::GpuLoadOp::Load => wgpu::LoadOp::Load,
                                },
                                store: attachment
                                    .stencil_store_op
                                    .map_or(wgpu::StoreOp::Store, Convert::convert),
                            }),
                        },
                    }
                });
                let encoder = found(gpu.encoders.get_mut(encoder, "command encoder"))?;
                Ok(encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: descriptor.label.as_deref(),
                        color_attachments: &color_attachments,
                        depth_stencil_attachment,
                        timestamp_writes: timestamps.as_ref().map(|(set, writes)| {
                            wgpu::RenderPassTimestampWrites {
                                query_set: set,
                                beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                                end_of_pass_write_index: writes.end_of_pass_write_index,
                            }
                        }),
                        occlusion_query_set: occlusion.as_ref(),
                        multiview_mask: None,
                    })
                    .forget_lifetime())
                .map(|pass| Recording::new(pass, device))
            },
        )
    }

    pub(super) fn begin_compute_pass(
        &mut self,
        encoder: u32,
        descriptor: Option<web::GpuComputePassDescriptor>,
    ) -> u32 {
        let device = self.owner(Kind::Encoder, encoder);
        let label = descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.label.clone());
        self.make(
            device,
            Kind::ComputePass,
            label.as_deref(),
            |gpu| &mut gpu.compute_passes,
            |gpu| {
                let timestamps = descriptor
                    .as_ref()
                    .and_then(|descriptor| descriptor.timestamp_writes)
                    .map(|writes| {
                        gpu.held(
                            &gpu.query_sets,
                            Kind::QuerySet,
                            writes.query_set.0,
                            "query set",
                        )
                        .map(|set| (set.set.clone(), writes))
                    })
                    .transpose()?;
                let encoder = found(gpu.encoders.get_mut(encoder, "command encoder"))?;
                Ok(encoder
                    .begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: label.as_deref(),
                        timestamp_writes: timestamps.as_ref().map(|(set, writes)| {
                            wgpu::ComputePassTimestampWrites {
                                query_set: set,
                                beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                                end_of_pass_write_index: writes.end_of_pass_write_index,
                            }
                        }),
                    })
                    .forget_lifetime())
                .map(|pass| Recording::new(pass, device))
            },
        )
    }

    /// Resolves queries into a buffer, timestamps converted to nanoseconds and coarsened to
    /// 100 µs (queries.rs).
    pub(super) fn resolve(
        &mut self,
        encoder: u32,
        set: u32,
        first: u32,
        count: u32,
        destination: u32,
        offset: u64,
    ) {
        self.record(encoder, |gpu| {
            let found_set = gpu.held(&gpu.query_sets, Kind::QuerySet, set, "query set")?;
            let (queries, timestamps) = (found_set.set.clone(), found_set.timestamps);
            let to = gpu
                .held(&gpu.buffers, Kind::Buffer, destination, "buffer")?
                .clone();
            let coarsen = timestamps && Coarsening::fits(&to, offset, count);
            let coarsening = coarsen.then(|| (gpu.coarsening().clone(), gpu.device.clone()));
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| {
                encoder.resolve_query_set(
                    &queries,
                    first..first.saturating_add(count),
                    &to,
                    offset,
                );
                if let Some((coarsening, device)) = coarsening {
                    coarsening.record(&device, encoder, &to, offset, count);
                }
            }))
        });
    }

    /// An async pipeline: made now, its answer settled at the next poll. A pipeline that fails
    /// is a `GPUPipelineError`, not an error in a scope.
    pub(super) fn made_async<T>(
        &mut self,
        device: u32,
        kind: Kind,
        label: Option<String>,
        table: fn(&mut Self) -> &mut Table<T>,
        act: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<u32, web::CreatePipelineError> {
        let refuse = |reason, message: String| web::CreatePipelineError {
            kind: web::CreatePipelineErrorKind::GpuPipelineError(reason),
            message,
        };
        if self.device_lost(device) {
            return Err(refuse(
                web::GpuPipelineErrorReason::Internal,
                "the device is lost".to_owned(),
            ));
        }
        let scopes: Vec<wgpu::ErrorScopeGuard> =
            [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal]
                .into_iter()
                .map(|filter| self.device.push_error_scope(filter))
                .collect();
        let made = act(self);
        let mut caught = None;
        for scope in scopes.into_iter().rev() {
            if let Some(message) = crate::caught(scope.pop()) {
                caught.get_or_insert(message);
            }
        }
        match (made, caught) {
            (Ok(value), None) => {
                let handle = table(self).insert(value);
                self.own(kind, handle, Some(device), label.as_deref());
                Ok(handle)
            }
            (Ok(_), Some(message)) => Err(refuse(web::GpuPipelineErrorReason::Validation, message)),
            (Err(failure), _) => Err(refuse(
                web::GpuPipelineErrorReason::Validation,
                failure.message,
            )),
        }
    }
}
