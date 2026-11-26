//! An app device's limits, which are lower than the host device's that wgpu checks against:
//! what the app's calls ask past them is refused here.

use super::Failure;

/// Refuses `value` past `limit`, named `what`.
pub(super) fn within(what: &str, value: u64, limit: u32) -> Result<(), Failure> {
    match value <= u64::from(limit) {
        true => Ok(()),
        false => Err(Failure::validation(format!(
            "{what} is {value}, past the device's limit of {limit}"
        ))),
    }
}

pub(super) fn texture(
    limits: &wgpu::Limits,
    descriptor: &wgpu::TextureDescriptor<'_>,
) -> Result<(), Failure> {
    let size = descriptor.size;
    match descriptor.dimension {
        wgpu::TextureDimension::D1 => within(
            "a 1D texture's width",
            size.width.into(),
            limits.max_texture_dimension_1d,
        ),
        wgpu::TextureDimension::D2 => {
            within(
                "a 2D texture's width",
                size.width.into(),
                limits.max_texture_dimension_2d,
            )?;
            within(
                "a 2D texture's height",
                size.height.into(),
                limits.max_texture_dimension_2d,
            )?;
            within(
                "a 2D texture's array layers",
                size.depth_or_array_layers.into(),
                limits.max_texture_array_layers,
            )
        }
        wgpu::TextureDimension::D3 => {
            within(
                "a 3D texture's width",
                size.width.into(),
                limits.max_texture_dimension_3d,
            )?;
            within(
                "a 3D texture's height",
                size.height.into(),
                limits.max_texture_dimension_3d,
            )?;
            within(
                "a 3D texture's depth",
                size.depth_or_array_layers.into(),
                limits.max_texture_dimension_3d,
            )
        }
    }
}

/// A bind group layout's bindings, counted per shader stage as WebGPU counts them.
pub(super) fn group_layout(
    limits: &wgpu::Limits,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> Result<(), Failure> {
    for entry in entries {
        if entry.binding >= limits.max_bindings_per_bind_group {
            return Err(Failure::validation(format!(
                "binding {} is past the device's limit of {} bindings per bind group",
                entry.binding, limits.max_bindings_per_bind_group
            )));
        }
    }
    let dynamic = |uniform: bool| {
        entries
            .iter()
            .filter(|entry| match entry.ty {
                wgpu::BindingType::Buffer {
                    ty,
                    has_dynamic_offset: true,
                    ..
                } => matches!(ty, wgpu::BufferBindingType::Uniform) == uniform,
                _ => false,
            })
            .count() as u64
    };
    within(
        "the dynamic uniform buffers of a bind group layout",
        dynamic(true),
        limits.max_dynamic_uniform_buffers_per_pipeline_layout,
    )?;
    within(
        "the dynamic storage buffers of a bind group layout",
        dynamic(false),
        limits.max_dynamic_storage_buffers_per_pipeline_layout,
    )?;
    for stage in [
        wgpu::ShaderStages::VERTEX,
        wgpu::ShaderStages::FRAGMENT,
        wgpu::ShaderStages::COMPUTE,
    ] {
        let count = |kind: fn(&wgpu::BindingType) -> bool| {
            entries
                .iter()
                .filter(|entry| entry.visibility.contains(stage) && kind(&entry.ty))
                .count() as u64
        };
        within(
            "a stage's sampled textures",
            count(|ty| matches!(ty, wgpu::BindingType::Texture { .. })),
            limits.max_sampled_textures_per_shader_stage,
        )?;
        within(
            "a stage's samplers",
            count(|ty| matches!(ty, wgpu::BindingType::Sampler(_))),
            limits.max_samplers_per_shader_stage,
        )?;
        within(
            "a stage's storage textures",
            count(|ty| matches!(ty, wgpu::BindingType::StorageTexture { .. })),
            limits.max_storage_textures_per_shader_stage,
        )?;
        within(
            "a stage's uniform buffers",
            count(|ty| {
                matches!(
                    ty,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        ..
                    }
                )
            }),
            limits.max_uniform_buffers_per_shader_stage,
        )?;
        within(
            "a stage's storage buffers",
            count(|ty| {
                matches!(
                    ty,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { .. },
                        ..
                    }
                )
            }),
            limits.max_storage_buffers_per_shader_stage,
        )?;
    }
    Ok(())
}

/// A render pipeline's vertex buffers and color targets.
pub(super) fn render_pipeline(
    limits: &wgpu::Limits,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    targets: usize,
) -> Result<(), Failure> {
    within(
        "a pipeline's vertex buffers",
        buffers.len() as u64,
        limits.max_vertex_buffers,
    )?;
    let attributes = buffers
        .iter()
        .flatten()
        .map(|buffer| buffer.attributes.len())
        .sum::<usize>();
    within(
        "a pipeline's vertex attributes",
        attributes as u64,
        limits.max_vertex_attributes,
    )?;
    for buffer in buffers.iter().flatten() {
        within(
            "a vertex buffer's stride",
            buffer.array_stride,
            limits.max_vertex_buffer_array_stride,
        )?;
    }
    within(
        "a pipeline's color targets",
        targets as u64,
        limits.max_color_attachments,
    )
}
