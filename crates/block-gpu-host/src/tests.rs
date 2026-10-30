use block_gpu_abi::embed::device::{Caller, Host, glue};

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
mod a_device_the_gpu_owns_tells_the_plugin_when_it_is_lost;
mod a_dropped_handle_is_reported_not_reused;
mod a_malformed_descriptor_is_refused;
mod a_mapped_range_is_read_raw_and_only_while_mapped;
mod a_rejected_shader_module_is_kept_and_says_why;
mod a_render_bundle_that_fails_validation_is_reported_not_panicked;
mod a_result_longer_than_its_room_waits_to_be_taken;
mod a_settled_map_wakes_the_plugin_without_its_polling;
mod a_surface_must_be_attached_before_it_is_acquired;
mod a_surface_of_no_size_is_reported;
mod a_surface_reports_how_many_presents_ago_its_texture_was_drawn;
mod a_texture_copy_is_recorded_on_its_encoder;
mod an_unknown_pass_handle_is_reported;
mod destroying_the_plugins_device_leaves_the_hosts_device_working;
mod every_webgpu_limit_of_the_device_is_forwarded;
mod only_a_texture_the_plugin_created_is_destroyed;
mod submitted_work_wakes_the_plugin_once_the_gpu_has_done_it;
