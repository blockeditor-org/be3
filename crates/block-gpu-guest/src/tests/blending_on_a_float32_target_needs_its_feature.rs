use super::*;

#[test]
fn blending_on_a_float32_target_needs_its_feature() {
    let target = |format, blend| wgpu::ColorTargetState {
        format,
        blend: Some(blend),
        write_mask: wgpu::ColorWrites::ALL,
    };
    let float = target(
        wgpu::TextureFormat::R32Float,
        wgpu::BlendState::ALPHA_BLENDING,
    );
    assert!(
        allowed(wgpu::Features::empty())
            .color_target(&float)
            .is_err()
    );
    assert_eq!(
        allowed(wgpu::Features::FLOAT32_BLENDABLE).color_target(&float),
        Ok(())
    );
    let dual = target(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        },
    );
    assert!(
        allowed(wgpu::Features::empty())
            .color_target(&dual)
            .is_err(),
        "a factor naming the second blend source needs dual-source blending"
    );
}
