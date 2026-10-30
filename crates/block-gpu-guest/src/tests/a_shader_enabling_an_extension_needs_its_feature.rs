use super::*;

#[test]
fn a_shader_enabling_an_extension_needs_its_feature() {
    let shader = "enable f16;\n@fragment fn main() -> @location(0) vec4<f16> { return vec4<f16>(1.0h); }";
    assert!(allowed(wgpu::Features::empty()).shader(shader).is_err());
    assert_eq!(allowed(wgpu::Features::SHADER_F16).shader(shader), Ok(()));
    assert_eq!(
        allowed(wgpu::Features::empty()).shader("@fragment fn main() {}"),
        Ok(())
    );
}
