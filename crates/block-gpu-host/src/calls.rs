use std::collections::VecDeque;

use block_gpu_abi as abi;

use crate::Gpu;

pub trait Guest {
    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String>;

    fn write(&mut self, pointer: u32, capacity: u32, bytes: &[u8]) -> u32;

    fn report(&mut self, message: String);

    fn gpu<R: Default>(&mut self, call: impl FnOnce(&mut Gpu) -> R) -> R;

    fn take_error(&mut self) -> Option<String>;

    fn transport(&mut self) -> &mut Transport;

    fn submit(&mut self, handles: &[u32]) {
        self.gpu(|gpu| gpu.submit(handles));
    }
}

#[derive(Default)]
pub struct Transport {
    pub inbox: VecDeque<Vec<u8>>,
    pub outbox: Vec<Vec<u8>>,
}

fn read(guest: &mut impl Guest, pointer: u32, length: u32) -> Option<Vec<u8>> {
    match guest.read(pointer, length) {
        Ok(bytes) => Some(bytes),
        Err(message) => {
            guest.report(message);
            None
        }
    }
}

fn read_words(guest: &mut impl Guest, pointer: u32, count: u32) -> Option<Vec<u32>> {
    let bytes = read(guest, pointer, count.saturating_mul(4))?;
    Some(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(u32::from_le_bytes)
            .collect(),
    )
}

macro_rules! described {
    ($($name:ident => $method:ident;)+) => {
        $(
            pub fn $name(guest: &mut impl Guest, pointer: u32, length: u32) -> u32 {
                match read(guest, pointer, length) {
                    Some(bytes) => guest.gpu(|gpu| gpu.$method(&bytes)),
                    None => abi::NULL_HANDLE,
                }
            }
        )+
    };
}

macro_rules! scalar {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)? => $method:ident;)+) => {
        $(
            pub fn $name(guest: &mut impl Guest, $($argument: $kind),*) $(-> $result)? {
                guest.gpu(|gpu| gpu.$method($($argument),*))
            }
        )+
    };
}

described! {
    create_buffer => create_buffer;
    create_texture => create_texture;
    create_texture_view => create_texture_view;
    create_sampler => create_sampler;
    create_bind_group_layout => create_bind_group_layout;
    create_bind_group => create_bind_group;
    create_pipeline_layout => create_pipeline_layout;
    create_shader_module => create_shader_module;
    create_render_pipeline => create_render_pipeline;
    create_command_encoder => create_command_encoder;
    encoder_begin_render_pass => begin_render_pass;
}

scalar! {
    fn buffer_unmap(buffer: u32) => unmap_buffer;
    fn encoder_finish(encoder: u32) -> u32 => finish_encoder;
    fn pass_set_pipeline(pass: u32, pipeline: u32) => set_pipeline;
    fn pass_set_index_buffer(pass: u32, buffer: u32, format: u32, offset: u64, size: u64)
        => set_index_buffer;
    fn pass_set_vertex_buffer(pass: u32, slot: u32, buffer: u32, offset: u64, size: u64)
        => set_vertex_buffer;
    fn pass_set_viewport(
        pass: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        minimum_depth: f32,
        maximum_depth: f32,
    ) => set_viewport;
    fn pass_set_scissor_rect(pass: u32, x: u32, y: u32, width: u32, height: u32)
        => set_scissor_rect;
    fn pass_set_blend_constant(pass: u32, red: f32, green: f32, blue: f32, alpha: f32)
        => set_blend_constant;
    fn pass_set_stencil_reference(pass: u32, reference: u32) => set_stencil_reference;
    fn pass_draw(
        pass: u32,
        first_vertex: u32,
        vertex_count: u32,
        first_instance: u32,
        instance_count: u32,
    ) => draw;
    fn pass_draw_indexed(
        pass: u32,
        first_index: u32,
        index_count: u32,
        base_vertex: i32,
        first_instance: u32,
        instance_count: u32,
    ) => draw_indexed;
    fn pass_end(pass: u32) => end_pass;
    fn resource_drop(kind: u32, handle: u32) => drop_resource;
    fn surface_acquire(surface: u32) -> u32 => acquire_surface;
    fn surface_age(surface: u32) -> u32 => surface_age;
    fn surface_present(surface: u32) => present_surface;
    fn surface_release(surface: u32) => detach_surface;
}

pub fn device_limits(guest: &mut impl Guest, pointer: u32, capacity: u32) -> u32 {
    let bytes = guest.gpu(|gpu| gpu.limits());
    guest.write(pointer, capacity, &bytes)
}

pub fn buffer_write_mapped(
    guest: &mut impl Guest,
    buffer: u32,
    offset: u64,
    pointer: u32,
    length: u32,
) {
    if let Some(data) = read(guest, pointer, length) {
        guest.gpu(|gpu| gpu.write_mapped_buffer(buffer, offset, &data));
    }
}

pub fn queue_write_buffer(
    guest: &mut impl Guest,
    buffer: u32,
    offset: u64,
    pointer: u32,
    length: u32,
) {
    if let Some(data) = read(guest, pointer, length) {
        guest.gpu(|gpu| gpu.write_buffer(buffer, offset, &data));
    }
}

pub fn queue_write_texture(
    guest: &mut impl Guest,
    pointer: u32,
    length: u32,
    data: u32,
    data_length: u32,
) {
    let Some(request) = read(guest, pointer, length) else {
        return;
    };
    let Some(payload) = read(guest, data, data_length) else {
        return;
    };
    guest.gpu(|gpu| gpu.write_texture(&request, &payload));
}

pub fn queue_submit(guest: &mut impl Guest, pointer: u32, length: u32) {
    if let Some(handles) = read_words(guest, pointer, length) {
        guest.submit(&handles);
    }
}

pub fn encoder_copy_texture_to_texture(guest: &mut impl Guest, pointer: u32, length: u32) {
    if let Some(bytes) = read(guest, pointer, length) {
        guest.gpu(|gpu| gpu.copy_texture_to_texture(&bytes));
    }
}

pub fn pass_set_bind_group(
    guest: &mut impl Guest,
    pass: u32,
    index: u32,
    bind_group: u32,
    offsets: u32,
    offsets_length: u32,
) {
    if let Some(offsets) = read_words(guest, offsets, offsets_length) {
        guest.gpu(|gpu| gpu.set_bind_group(pass, index, bind_group, &offsets));
    }
}

pub fn surface_configure(guest: &mut impl Guest, surface: u32, pointer: u32, length: u32) {
    if let Some(bytes) = read(guest, pointer, length) {
        guest.gpu(|gpu| gpu.configure_surface(surface, &bytes));
    }
}

pub fn texture_describe(guest: &mut impl Guest, texture: u32, pointer: u32, capacity: u32) -> u32 {
    match guest.gpu(|gpu| gpu.describe_texture(texture)) {
        Some(bytes) => guest.write(pointer, capacity, &bytes),
        None => 0,
    }
}

pub fn error_take(guest: &mut impl Guest, pointer: u32, capacity: u32) -> u32 {
    let message = guest.take_error().unwrap_or_default();
    guest.write(pointer, capacity, message.as_bytes())
}

pub fn host_send(guest: &mut impl Guest, pointer: u32, length: u32) {
    if let Some(frame) = read(guest, pointer, length) {
        guest.transport().outbox.push(frame);
    }
}

pub fn host_receive(guest: &mut impl Guest, pointer: u32, capacity: u32) -> i64 {
    let Some(needed) = guest.transport().inbox.front().map(Vec::len) else {
        return block_guest_abi::NO_MESSAGE;
    };
    if needed as u32 > capacity {
        return needed as i64;
    }
    let frame = guest.transport().inbox.pop_front().unwrap_or_default();
    guest.write(pointer, capacity, &frame);
    needed as i64
}
