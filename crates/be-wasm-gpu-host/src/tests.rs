use be_wasm_gpu_abi::embed::webgpu::{Caller, glue};

use super::*;

fn gpu() -> Gpu {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Gpu::new(device, queue)
}

/// A gpu that tells the receiver each time it wakes the app.
fn waking() -> (Gpu, std::sync::mpsc::Receiver<()>) {
    let (woken, wakes) = std::sync::mpsc::channel();
    let woken = Mutex::new(woken);
    let gpu = gpu().waking(Arc::new(move || {
        let _ = woken.lock().unwrap().send(());
    }));
    (gpu, wakes)
}

struct Guest {
    memory: Vec<u8>,
    gpu: Gpu,
    refused: Vec<String>,
    returned: Vec<u8>,
}

impl Guest {
    fn new() -> Self {
        Self {
            memory: vec![0; 4096],
            gpu: gpu(),
            refused: Vec::new(),
            returned: Vec::new(),
        }
    }

    fn place(&mut self, at: u32, bytes: &[u8]) -> (u32, u32) {
        self.memory[at as usize..at as usize + bytes.len()].copy_from_slice(bytes);
        (at, bytes.len() as u32)
    }
}

impl Caller for Guest {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let range = pointer as usize..pointer as usize + length as usize;
        self.memory
            .get(range)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "a pointer ran past its memory".to_owned())
    }

    fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        let range = pointer as usize..pointer as usize + bytes.len();
        self.memory
            .get_mut(range)
            .ok_or_else(|| "a pointer ran past its memory".to_owned())?
            .copy_from_slice(bytes);
        Ok(())
    }

    fn host<R>(&mut self, _function: &'static str, call: impl FnOnce(&mut Gpu) -> R) -> Option<R> {
        Some(call(&mut self.gpu))
    }

    fn refuse(&mut self, message: String) {
        self.refused.push(message);
    }

    fn returned(&mut self) -> &mut Vec<u8> {
        &mut self.returned
    }

    fn ticket(&mut self) -> u32 {
        self.gpu.webgpu_ticket()
    }
}

mod a_buffer_range_past_the_end_of_its_buffer_is_reported;
mod a_canvas_answers_its_configuration_with_the_defaults_filled_in;
mod a_canvas_configured_or_resized_again_is_shown_as_it_is_now;
mod a_canvas_is_shown_as_its_configuration_asks;
mod a_compute_pass_is_recorded_on_its_encoder;
mod a_configured_surface_alternates_between_two_textures;
mod a_configured_surface_keeps_its_texture_until_its_size_changes;
mod a_device_offers_only_the_features_browser_webgpu_has;
mod a_device_the_gpu_owns_tells_the_app_when_it_is_lost;
mod a_dropped_handle_is_reported_not_reused;
mod a_malformed_descriptor_is_refused;
mod a_map_cut_short_settles_failed_once_and_never_settles_a_later_one;
mod a_mapped_range_is_read_raw_and_only_while_mapped;
mod a_new_instances_surfaces_never_share_a_generation_with_an_old_ones;
mod a_rejected_shader_module_is_kept_and_says_why;
mod a_render_bundle_that_fails_validation_is_reported_not_panicked;
mod a_result_longer_than_its_room_waits_to_be_taken;
mod a_settled_map_wakes_the_app_without_its_polling;
mod a_surface_configured_alike_keeps_its_textures_however_its_view_formats_are_named;
mod a_surface_must_be_attached_before_it_is_acquired;
mod a_surface_reports_how_many_presents_ago_its_texture_was_drawn;
mod a_surface_shows_what_was_presented_until_it_is_presented_again;
mod a_texture_copy_is_recorded_on_its_encoder;
mod a_webgpu_canvas_has_textures_once_resized_and_configured;
mod a_webgpu_canvas_its_device_cannot_make_is_refused;
mod a_webgpu_device_has_only_what_it_asked_for;
mod a_webgpu_error_goes_to_the_innermost_scope_of_its_kind;
mod a_webgpu_limit_asked_worse_than_the_default_keeps_the_default;
mod a_wrapped_webgpu_ticket_skips_one_still_held;
mod an_imported_texture_is_only_read;
mod an_object_of_one_webgpu_device_is_refused_on_another;
mod an_uncaptured_webgpu_error_reaches_the_listener_or_the_log;
mod an_unknown_pass_handle_is_reported;
mod destroying_the_apps_device_leaves_the_hosts_device_working;
mod every_webgpu_limit_of_the_device_is_forwarded;
mod only_a_texture_the_app_created_is_destroyed;
mod only_the_calls_that_record_into_a_pass_go_untimed;
mod several_webgpu_devices_are_lost_apart;
mod submitted_work_wakes_the_app_once_the_gpu_has_done_it;
mod unbinding_a_vertex_buffer_takes_no_offset_or_size;
mod webgpu_maps_and_work_settle_through_the_poll;

/// wasi:webgpu's side of the host: a device requested as an app would, and what its promises
/// settle to.
mod web {
    pub(super) use be_wasm_gpu_abi::embed::host::Host as _;
    pub(super) use be_wasm_gpu_abi::embed::webgpu::Host as _;
    use be_wasm_gpu_abi::embed::webgpu::Host;
    pub(super) use be_wasm_gpu_abi::webgpu::webgpu::*;
    use be_wasm_ops::local::{Promise, Settled};
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::Gpu;

    static TICKETS: AtomicU32 = AtomicU32::new(5000);

    /// A ticket no other promise of the test holds.
    pub(super) fn ticket() -> u32 {
        TICKETS.fetch_add(1, Ordering::Relaxed)
    }

    pub(super) const TICKET: u32 = 1000;

    /// The value that settled for `ticket` in the next poll.
    pub(super) fn settled<T: be_wasm_ops::DeserializeOwned>(gpu: &mut Gpu, ticket: u32) -> T {
        answered(gpu, ticket, true)
    }

    /// The value that settled for `ticket` in a poll that does not wait: what woke the app.
    pub(super) fn settled_now<T: be_wasm_ops::DeserializeOwned>(gpu: &mut Gpu, ticket: u32) -> T {
        answered(gpu, ticket, false)
    }

    fn answered<T: be_wasm_ops::DeserializeOwned>(gpu: &mut Gpu, ticket: u32, wait: bool) -> T {
        let settled: Vec<Settled> = Host::settle(gpu, wait);
        let value = settled
            .into_iter()
            .find(|settled| settled.ticket == ticket)
            .and_then(|settled| settled.value)
            .unwrap_or_else(|| panic!("ticket {ticket} did not settle"));
        be_wasm_ops::decode(&value).expect("what settled decodes")
    }

    pub(super) fn device(
        gpu: &mut Gpu,
        features: Vec<GpuFeatureName>,
    ) -> Result<GpuDevice, RequestDeviceError> {
        let instance = gpu.get_gpu();
        gpu.gpu_request_adapter(instance, None, Promise::new(TICKET));
        let adapter: Option<GpuAdapter> = settled(gpu, TICKET);
        let descriptor = GpuDeviceDescriptor {
            required_features: Some(features),
            required_limits: None,
            default_queue: None,
            label: Some("app".into()),
        };
        gpu.gpu_adapter_request_device(
            adapter.expect("the host has an adapter"),
            Some(descriptor),
            Promise::new(TICKET + 1),
        );
        settled(gpu, TICKET + 1)
    }

    pub(super) fn buffer(usage: GpuBufferUsage) -> GpuBufferDescriptor {
        GpuBufferDescriptor {
            size: 256,
            usage,
            mapped_at_creation: None,
            label: None,
        }
    }

    /// Pops the innermost error scope and answers the error it caught.
    pub(super) fn pop(
        gpu: &mut Gpu,
        device: GpuDevice,
        ticket: u32,
    ) -> Result<Option<GpuError>, PopErrorScopeError> {
        gpu.gpu_device_pop_error_scope(device, Promise::new(ticket));
        settled(gpu, ticket)
    }

    /// What a validation scope around `act` caught on `device`, as its message.
    pub(super) fn scoped(
        gpu: &mut Gpu,
        device: GpuDevice,
        act: impl FnOnce(&mut Gpu),
    ) -> Option<String> {
        gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
        act(gpu);
        let error = pop(gpu, device, ticket()).expect("the scope pops")?;
        Some(gpu.gpu_error_message(error))
    }

    /// Configures `context` as a premultiplied rgba8unorm canvas of `device`, viewed also as
    /// `views`: one shown as it is, as be-sdk's are.
    pub(super) fn configure(
        gpu: &mut Gpu,
        device: GpuDevice,
        context: GpuCanvasContext,
        views: Vec<GpuTextureFormat>,
    ) {
        gpu.gpu_canvas_context_configure(
            context,
            GpuCanvasConfiguration {
                device,
                format: GpuTextureFormat::Rgba8unorm,
                usage: None,
                view_formats: Some(views),
                color_space: None,
                tone_mapping: None,
                alpha_mode: Some(GpuCanvasAlphaMode::Premultiplied),
            },
        );
    }

    /// The context of `surface`, sized and configured.
    pub(super) fn canvas(
        gpu: &mut Gpu,
        device: GpuDevice,
        surface: u32,
        (width, height): (u32, u32),
        views: Vec<GpuTextureFormat>,
    ) -> GpuCanvasContext {
        let context = be_wasm_gpu_abi::embed::host::Host::surface(gpu, surface);
        gpu.resize(context, width, height);
        configure(gpu, device, context, views);
        context
    }

    /// The texture of `context`'s frame in progress.
    pub(super) fn current(gpu: &mut Gpu, context: GpuCanvasContext) -> wgpu::Texture {
        let texture = gpu.gpu_canvas_context_get_current_texture(context);
        gpu.textures.get(texture.0, "texture").unwrap().clone()
    }

    pub(super) fn module(gpu: &mut Gpu, device: GpuDevice, code: &str) -> GpuShaderModule {
        gpu.gpu_device_create_shader_module(
            device,
            GpuShaderModuleDescriptor {
                code: code.to_owned(),
                compilation_hints: None,
                label: None,
            },
        )
    }

    /// A render bundle encoder for one rgba8unorm target.
    pub(super) fn bundling(gpu: &mut Gpu, device: GpuDevice) -> GpuRenderBundleEncoder {
        gpu.gpu_device_create_render_bundle_encoder(
            device,
            GpuRenderBundleEncoderDescriptor {
                color_formats: vec![Some(GpuTextureFormat::Rgba8unorm)],
                depth_stencil_format: None,
                sample_count: None,
                depth_read_only: None,
                stencil_read_only: None,
                label: None,
            },
        )
    }

    /// An 8 by 8 rgba8unorm texture.
    pub(super) fn texture(gpu: &mut Gpu, device: GpuDevice, usage: GpuTextureUsage) -> GpuTexture {
        gpu.gpu_device_create_texture(
            device,
            GpuTextureDescriptor {
                size: GpuExtent3D {
                    width: 8,
                    height: Some(8),
                    depth_or_array_layers: None,
                },
                mip_level_count: None,
                sample_count: None,
                dimension: None,
                format: GpuTextureFormat::Rgba8unorm,
                usage,
                view_formats: None,
                texture_binding_view_dimension: None,
                label: None,
            },
        )
    }
}

/// Canvases on a real device (lavapipe), drawn and read back as the host shows them.
mod looking {
    use super::Gpu;
    use super::web::*;

    /// A gpu on a real device, which draws what the host shows.
    pub(super) fn drawing() -> Gpu {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("the test has lavapipe");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("lavapipe gives a device");
        Gpu::new(device, queue)
    }

    /// A canvas of `format` on surface 0 of a gpu on a real device, as `configure` changes its
    /// configuration.
    pub(super) fn drawn_canvas(
        format: GpuTextureFormat,
        configure: impl FnOnce(&mut GpuCanvasConfiguration),
    ) -> (Gpu, GpuDevice, GpuCanvasContext) {
        let mut gpu = drawing();
        let device = device(&mut gpu, Vec::new()).unwrap();
        let context = be_wasm_gpu_abi::embed::host::Host::surface(&mut gpu, 0);
        gpu.resize(context, 4, 4);
        reconfigure(&mut gpu, device, context, format, configure);
        (gpu, device, context)
    }

    pub(super) fn reconfigure(
        gpu: &mut Gpu,
        device: GpuDevice,
        context: GpuCanvasContext,
        format: GpuTextureFormat,
        configure: impl FnOnce(&mut GpuCanvasConfiguration),
    ) {
        let mut configuration = GpuCanvasConfiguration {
            device,
            format,
            usage: None,
            view_formats: None,
            color_space: None,
            tone_mapping: None,
            alpha_mode: None,
        };
        configure(&mut configuration);
        gpu.gpu_canvas_context_configure(context, configuration);
    }

    /// What surface 0 shows once its canvas is cleared to `clear` and presented and the host took
    /// what was presented: its first pixel, and whether it is the canvas's own texture.
    pub(super) fn shown(gpu: &mut Gpu, context: GpuCanvasContext, clear: wgpu::Color) -> ([u8; 4], bool) {
        let canvas = current(gpu, context);
        let mut encoder = gpu.device().create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &canvas.create_view(&Default::default()),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        gpu.queue().submit([encoder.finish()]);
        gpu.present(context);
        assert_eq!(gpu.take_presented(), vec![0]);
        let (texture, _) = gpu.presented_surface(0).expect("the canvas was presented");
        let texture = texture.clone();
        let readable = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * u64::from(texture.height()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device().create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readable,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        gpu.queue().submit([encoder.finish()]);
        readable.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device().poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let pixel: [u8; 4] = readable.slice(..4).get_mapped_range().unwrap()[..4]
            .try_into()
            .unwrap();
        let logged = gpu.take_logged();
        assert!(logged.is_empty(), "{logged:?}");
        (pixel, texture == canvas)
    }

    pub(super) fn near(shown: [u8; 4], wanted: [u8; 4]) -> bool {
        shown.iter().zip(wanted).all(|(shown, wanted)| shown.abs_diff(wanted) <= 2)
    }

    pub(super) fn premultiplied(configuration: &mut GpuCanvasConfiguration) {
        configuration.alpha_mode = Some(GpuCanvasAlphaMode::Premultiplied);
    }

    pub(super) const GRAY: wgpu::Color = wgpu::Color { r: 0.2, g: 0.2, b: 0.2, a: 0.5 };
}
