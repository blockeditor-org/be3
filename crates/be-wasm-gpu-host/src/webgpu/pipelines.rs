//! Pipelines: what one is made of, looked up when it is asked for, and made at once
//! (`create-*-pipeline`) or, for `create-*-pipeline-async`, off the app's thread natively,
//! settled through the poll once made. A browser has no threads for it (the shim makes it at
//! once, settled at the next poll).

use std::sync::mpsc;

use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::Convert;
use be_wasm_gpu_convert::allowed::Allowed;
use be_wasm_ops::local::Settled;

use super::{Failure, Kind, limits};
use crate::Gpu;

/// What a render pipeline is made of, owned, so that it can be made on another thread.
pub(crate) struct RenderParts {
    pub(super) descriptor: web::GpuRenderPipelineDescriptor,
    pub(super) layout: Option<wgpu::PipelineLayout>,
    pub(super) vertex_module: wgpu::ShaderModule,
    pub(super) fragment_module: Option<wgpu::ShaderModule>,
    pub(super) vertex_constants: Vec<(String, f64)>,
    pub(super) fragment_constants: Vec<(String, f64)>,
    pub(super) allowed: Allowed,
    pub(super) limits: Option<wgpu::Limits>,
}

impl RenderParts {
    pub(super) fn make(self, device: &wgpu::Device) -> Result<wgpu::RenderPipeline, Failure> {
        let layouts = self.descriptor.vertex.buffers.clone().unwrap_or_default();
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = layouts
            .iter()
            .map(|layout| {
                layout
                    .iter()
                    .flat_map(|layout| &layout.attributes)
                    .map(|attribute| wgpu::VertexAttribute {
                        format: attribute.format.convert(),
                        offset: attribute.offset,
                        shader_location: attribute.shader_location,
                    })
                    .collect()
            })
            .collect();
        let buffers: Vec<_> = layouts
            .iter()
            .zip(&attributes)
            .map(|(layout, attributes)| {
                layout.as_ref().map(|layout| wgpu::VertexBufferLayout {
                    array_stride: layout.array_stride,
                    step_mode: layout
                        .step_mode
                        .map_or(wgpu::VertexStepMode::Vertex, Convert::convert),
                    attributes,
                })
            })
            .collect();
        let targets: Vec<Option<wgpu::ColorTargetState>> = self.descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.as_ref().map(|target| wgpu::ColorTargetState {
                            format: target.format.convert(),
                            blend: target.blend.map(|blend| wgpu::BlendState {
                                color: blend.color.convert(),
                                alpha: blend.alpha.convert(),
                            }),
                            write_mask: target
                                .write_mask
                                .map_or(wgpu::ColorWrites::ALL, Convert::convert),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(limits) = &self.limits {
            limits::render_pipeline(limits, &buffers, targets.len())?;
        }
        let primitive = self.descriptor.primitive.unwrap_or(web::GpuPrimitiveState {
            topology: None,
            strip_index_format: None,
            front_face: None,
            cull_mode: None,
            unclipped_depth: None,
        });
        let multisample = self.descriptor.multisample.unwrap_or(web::GpuMultisampleState {
            count: None,
            mask: None,
            alpha_to_coverage_enabled: None,
        });
        let vertex_constants: Vec<(&str, f64)> = self
            .vertex_constants
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        let fragment_constants: Vec<(&str, f64)> = self
            .fragment_constants
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        let wgpu_descriptor = wgpu::RenderPipelineDescriptor {
            label: self.descriptor.label.as_deref(),
            layout: self.layout.as_ref(),
            vertex: wgpu::VertexState {
                module: &self.vertex_module,
                entry_point: self.descriptor.vertex.entry_point.as_deref(),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &vertex_constants,
                    ..Default::default()
                },
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: primitive
                    .topology
                    .map_or(wgpu::PrimitiveTopology::TriangleList, Convert::convert),
                strip_index_format: primitive.strip_index_format.map(Convert::convert),
                front_face: primitive
                    .front_face
                    .map_or(wgpu::FrontFace::Ccw, Convert::convert),
                cull_mode: match primitive.cull_mode {
                    None | Some(web::GpuCullMode::None) => None,
                    Some(web::GpuCullMode::Front) => Some(wgpu::Face::Front),
                    Some(web::GpuCullMode::Back) => Some(wgpu::Face::Back),
                },
                unclipped_depth: primitive.unclipped_depth.unwrap_or(false),
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: self.descriptor
                .depth_stencil
                .map(|state| wgpu::DepthStencilState {
                    format: state.format.convert(),
                    depth_write_enabled: state.depth_write_enabled,
                    depth_compare: state.depth_compare.map(Convert::convert),
                    stencil: wgpu::StencilState {
                        front: state.stencil_front.convert(),
                        back: state.stencil_back.convert(),
                        read_mask: state.stencil_read_mask.unwrap_or(u32::MAX),
                        write_mask: state.stencil_write_mask.unwrap_or(u32::MAX),
                    },
                    bias: wgpu::DepthBiasState {
                        constant: state.depth_bias.unwrap_or(0),
                        slope_scale: state.depth_bias_slope_scale.unwrap_or(0.0),
                        clamp: state.depth_bias_clamp.unwrap_or(0.0),
                    },
                }),
            multisample: wgpu::MultisampleState {
                count: multisample.count.unwrap_or(1),
                mask: u64::from(multisample.mask.unwrap_or(u32::MAX)),
                alpha_to_coverage_enabled: multisample.alpha_to_coverage_enabled.unwrap_or(false),
            },
            fragment: self.fragment_module.as_ref().map(|module| wgpu::FragmentState {
                module,
                entry_point: self.descriptor
                    .fragment
                    .as_ref()
                    .and_then(|fragment| fragment.entry_point.as_deref()),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &fragment_constants,
                    ..Default::default()
                },
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        };
        self.allowed
            .render_pipeline(&wgpu_descriptor)
            .map_err(Failure::validation)?;
        Ok(device.create_render_pipeline(&wgpu_descriptor))
    }
}

/// What a compute pipeline is made of, as `RenderParts`.
pub(crate) struct ComputeParts {
    pub(super) descriptor: web::GpuComputePipelineDescriptor,
    pub(super) layout: Option<wgpu::PipelineLayout>,
    pub(super) module: wgpu::ShaderModule,
    pub(super) constants: Vec<(String, f64)>,
}

impl ComputeParts {
    pub(super) fn make(self, device: &wgpu::Device) -> Result<wgpu::ComputePipeline, Failure> {
        let constants: Vec<(&str, f64)> = self
            .constants
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        Ok(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: self.descriptor.label.as_deref(),
            layout: self.layout.as_ref(),
            module: &self.module,
            entry_point: self.descriptor.compute.entry_point.as_deref(),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            },
            cache: None,
        }))
    }
}

/// A pipeline to make.
pub(crate) enum Job {
    Render(Box<RenderParts>),
    Compute(Box<ComputeParts>),
}

enum Made {
    Render(wgpu::RenderPipeline),
    Compute(wgpu::ComputePipeline),
}

type Answer = Result<Made, (web::GpuPipelineErrorReason, String)>;

/// Settles an async pipeline's promise with its handle or why it was not made.
pub(crate) type Settle = Box<dyn FnOnce(Result<u32, web::CreatePipelineError>) -> Settled>;

/// A pipeline being made for an app device's promise.
pub(crate) struct Making {
    pub(super) ticket: u32,
    device: u32,
    label: Option<String>,
    made: mpsc::Receiver<Answer>,
    settle: Settle,
}

impl Job {
    /// Makes the pipeline inside scopes of its own, which on a native device are the making
    /// thread's alone, so they catch only what this pipeline got wrong.
    fn made(self, device: &wgpu::Device) -> Answer {
        let scopes: Vec<wgpu::ErrorScopeGuard> =
            [wgpu::ErrorFilter::Validation, wgpu::ErrorFilter::Internal]
                .into_iter()
                .map(|filter| device.push_error_scope(filter))
                .collect();
        let made = match self {
            Job::Render(parts) => parts.make(device).map(Made::Render),
            Job::Compute(parts) => parts.make(device).map(Made::Compute),
        };
        // Popped innermost first: an internal error before a validation one.
        let reasons = [
            web::GpuPipelineErrorReason::Internal,
            web::GpuPipelineErrorReason::Validation,
        ];
        let mut caught = None;
        for (scope, reason) in scopes.into_iter().rev().zip(reasons) {
            if let Some(message) = crate::caught(scope.pop()) {
                caught.get_or_insert((reason, message));
            }
        }
        match (made, caught) {
            (Ok(made), None) => Ok(made),
            (Ok(_), Some(caught)) => Err(caught),
            (Err(failure), _) => Err((web::GpuPipelineErrorReason::Validation, failure.message)),
        }
    }
}

fn refused(reason: web::GpuPipelineErrorReason, message: String) -> web::CreatePipelineError {
    web::CreatePipelineError {
        kind: web::CreatePipelineErrorKind::GpuPipelineError(reason),
        message,
    }
}

impl Gpu {
    /// Makes `job`'s pipeline for `device`'s promise: on a thread of its own natively, settled
    /// by the poll once made (the thread wakes the app); at once in a browser.
    pub(super) fn compile(
        &mut self,
        device: u32,
        label: Option<String>,
        job: Result<Job, Failure>,
        (ticket, settle): (u32, Settle),
    ) {
        let job = match job {
            _ if self.device_lost(device) => {
                let lost = refused(web::GpuPipelineErrorReason::Internal, "the device is lost".to_owned());
                self.web.settled.push(settle(Err(lost)));
                return self.settling.wake();
            }
            Ok(job) => job,
            Err(failure) => {
                let refusal = refused(web::GpuPipelineErrorReason::Validation, failure.message);
                self.web.settled.push(settle(Err(refusal)));
                return self.settling.wake();
            }
        };
        let (answer, made) = mpsc::channel();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let gpu = self.device.clone();
            let waker = self.settling.waker();
            let spawned = std::thread::Builder::new()
                .name("pipeline".to_owned())
                .spawn(move || {
                    let _ = answer.send(job.made(&gpu));
                    waker.wake();
                });
            if let Err(error) = spawned {
                let refusal = refused(
                    web::GpuPipelineErrorReason::Internal,
                    format!("no thread could make the pipeline: {error}"),
                );
                self.web.settled.push(settle(Err(refusal)));
                return self.settling.wake();
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = answer.send(job.made(&self.device));
            self.settling.wake();
        }
        self.web.making.push(Making {
            ticket,
            device,
            label,
            made,
            settle,
        });
    }

    /// Settles the pipelines made since the last poll; with `wait`, blocks until one is, when
    /// any is being made.
    pub(super) fn settle_pipelines(&mut self, wait: bool) {
        if wait && let Some(first) = self.web.making.first() {
            let answer = first.made.recv().map_err(|_| ());
            let making = self.web.making.remove(0);
            self.settle_pipeline(making, answer);
        }
        for making in std::mem::take(&mut self.web.making) {
            match making.made.try_recv() {
                Ok(answer) => self.settle_pipeline(making, Ok(answer)),
                Err(mpsc::TryRecvError::Empty) => self.web.making.push(making),
                Err(mpsc::TryRecvError::Disconnected) => self.settle_pipeline(making, Err(())),
            }
        }
    }

    fn settle_pipeline(&mut self, making: Making, answer: Result<Answer, ()>) {
        let Making {
            device,
            label,
            settle,
            ..
        } = making;
        let answer = match answer {
            _ if self.device_lost(device) => Err(refused(
                web::GpuPipelineErrorReason::Internal,
                "the device is lost".to_owned(),
            )),
            Err(()) => Err(refused(
                web::GpuPipelineErrorReason::Internal,
                "the thread making the pipeline stopped".to_owned(),
            )),
            Ok(Err((reason, message))) => Err(refused(reason, message)),
            Ok(Ok(Made::Render(pipeline))) => {
                let handle = self.pipelines.insert(pipeline);
                self.own(Kind::RenderPipeline, handle, Some(device), label.as_deref());
                Ok(handle)
            }
            Ok(Ok(Made::Compute(pipeline))) => {
                let handle = self.compute_pipelines.insert(pipeline);
                self.own(Kind::ComputePipeline, handle, Some(device), label.as_deref());
                Ok(handle)
            }
        };
        self.web.settled.push(settle(answer));
    }
}
