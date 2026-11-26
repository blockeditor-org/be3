use be_wasm_gpu_abi::embed::device::{Caller, Host, glue};

use super::*;

fn gpu() -> Gpu {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Gpu::new(device, queue)
}

fn buffer_descriptor() -> abi::BufferDescriptor {
    abi::BufferDescriptor {
        label: Some("vertices".into()),
        size: 256,
        usage: wgpu::BufferUsages::VERTEX.bits() | wgpu::BufferUsages::COPY_DST.bits(),
        mapped_at_creation: false,
    }
}

fn configuration(width: u32, height: u32) -> abi::SurfaceConfiguration {
    abi::SurfaceConfiguration {
        width,
        height,
        format: abi::TextureFormat::Rgba8Unorm,
        view_formats: Vec::new(),
    }
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
}

mod a_buffer_range_past_the_end_of_its_buffer_is_reported;
mod a_compute_pass_is_recorded_on_its_encoder;
mod a_configured_surface_alternates_between_two_textures;
mod a_configured_surface_keeps_its_texture_until_its_size_changes;
mod a_created_buffer_gets_a_live_handle;
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
mod a_surface_of_no_size_is_reported;
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
    pub(super) use be_wasm_gpu_abi::embed::webgpu::Host as _;
    use be_wasm_gpu_abi::embed::webgpu::Host;
    pub(super) use be_wasm_gpu_abi::webgpu::webgpu::*;
    use be_wasm_ops::local::{Promise, Settled};

    use super::Gpu;

    pub(super) const TICKET: u32 = 1000;

    /// The value that settled for `ticket` in the next poll.
    pub(super) fn settled<T: be_wasm_ops::DeserializeOwned>(gpu: &mut Gpu, ticket: u32) -> T {
        let settled: Vec<Settled> = Host::settle(gpu, true);
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
}
