//! Conversions between wasi:webgpu's types (be-wasm-gpu-abi) and wgpu's, which the guest's
//! backend (be-wasm-gpu-guest) and the host (be-wasm-gpu-host) share. A crate of its own, so the
//! ABI reaches a pure guest without wgpu: wgpu's `web` feature, which be-wasm-gpu-guest turns on
//! and cargo unifies into every guest's wgpu, links js-sys, web-sys and wasm-bindgen
//! (guides/app_libraries.md, Pure guests). They are traits of this crate's, since neither side's
//! types are.

/// `self` as the other side's `T`.
pub trait Convert<T> {
    fn convert(self) -> T;
}

/// `self` as the other side's `T`, or `self` back if `T` has nothing like it.
pub trait TryConvert<T>: Sized {
    fn try_convert(self) -> Result<T, Self>;
}

macro_rules! plain_map {
    ($abi:path, $wgpu:path, { $($variant:ident),* $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        impl Convert<$wgpu> for $abi {
            fn convert(self) -> $wgpu {
                let value = self;
                use $abi as Abi;
                use $wgpu as Wgpu;
                match value {
                    $(Abi::$variant => Wgpu::$variant,)*
                    $($(Abi::$named => $value,)+)?
                }
            }
        }

        impl TryConvert<$abi> for $wgpu {
            #[allow(unreachable_patterns)]
            fn try_convert(self) -> Result<$abi, $wgpu> {
                let value = self;
                use $abi as Abi;
                use $wgpu as Wgpu;
                match value {
                    $(Wgpu::$variant => Ok(Abi::$variant),)*
                    other => {
                        $($(if other == $value {
                            return Ok(Abi::$named);
                        })+)?
                        Err(other)
                    }
                }
            }
        }
    };
}

const fn astc(block: wgpu::AstcBlock, srgb: bool) -> wgpu::TextureFormat {
    wgpu::TextureFormat::Astc {
        block,
        channel: match srgb {
            true => wgpu::AstcChannel::UnormSrgb,
            false => wgpu::AstcChannel::Unorm,
        },
    }
}

pub mod allowed;
pub mod webgpu;

#[cfg(test)]
mod tests;

/// Calls `$then!` with every limit wgpu has a field for, by its WebGPU name: the one list the
/// host and the guest map limits by.
#[macro_export]
macro_rules! each_limit {
    ($then:ident) => {
        $then! {
            "maxTextureDimension1D" => max_texture_dimension_1d,
            "maxTextureDimension2D" => max_texture_dimension_2d,
            "maxTextureDimension3D" => max_texture_dimension_3d,
            "maxTextureArrayLayers" => max_texture_array_layers,
            "maxBindGroups" => max_bind_groups,
            "maxBindGroupsPlusVertexBuffers" => max_bind_groups_plus_vertex_buffers,
            "maxBindingsPerBindGroup" => max_bindings_per_bind_group,
            "maxDynamicUniformBuffersPerPipelineLayout" => max_dynamic_uniform_buffers_per_pipeline_layout,
            "maxDynamicStorageBuffersPerPipelineLayout" => max_dynamic_storage_buffers_per_pipeline_layout,
            "maxSampledTexturesPerShaderStage" => max_sampled_textures_per_shader_stage,
            "maxSamplersPerShaderStage" => max_samplers_per_shader_stage,
            "maxStorageBuffersPerShaderStage" => max_storage_buffers_per_shader_stage,
            "maxStorageTexturesPerShaderStage" => max_storage_textures_per_shader_stage,
            "maxUniformBuffersPerShaderStage" => max_uniform_buffers_per_shader_stage,
            "maxUniformBufferBindingSize" => max_uniform_buffer_binding_size,
            "maxStorageBufferBindingSize" => max_storage_buffer_binding_size,
            "minUniformBufferOffsetAlignment" => min_uniform_buffer_offset_alignment,
            "minStorageBufferOffsetAlignment" => min_storage_buffer_offset_alignment,
            "maxVertexBuffers" => max_vertex_buffers,
            "maxBufferSize" => max_buffer_size,
            "maxVertexAttributes" => max_vertex_attributes,
            "maxVertexBufferArrayStride" => max_vertex_buffer_array_stride,
            "maxInterStageShaderVariables" => max_inter_stage_shader_variables,
            "maxColorAttachments" => max_color_attachments,
            "maxColorAttachmentBytesPerSample" => max_color_attachment_bytes_per_sample,
            "maxComputeWorkgroupStorageSize" => max_compute_workgroup_storage_size,
            "maxComputeInvocationsPerWorkgroup" => max_compute_invocations_per_workgroup,
            "maxComputeWorkgroupSizeX" => max_compute_workgroup_size_x,
            "maxComputeWorkgroupSizeY" => max_compute_workgroup_size_y,
            "maxComputeWorkgroupSizeZ" => max_compute_workgroup_size_z,
            "maxComputeWorkgroupsPerDimension" => max_compute_workgroups_per_dimension,
            "maxImmediateSize" => max_immediate_size,
        }
    };
}
