use super::*;

fn acquired(gpu: &mut Gpu, surface: abi::Surface) -> wgpu::Texture {
    let handle = gpu
        .surface_acquire(surface)
        .expect("the configured surface should be acquired");
    gpu.textures
        .get(handle.0, "texture")
        .expect("the acquired surface should be a texture")
        .clone()
}

#[test]
fn a_configured_surface_alternates_between_two_textures() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    gpu.surface_configure(surface, configuration(320, 200));
    let first = acquired(&mut gpu, surface);
    gpu.surface_present(surface);
    let (shown, first_generation) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!(shown, &first);
    let second = acquired(&mut gpu, surface);
    assert_ne!(second, first);
    assert_eq!(gpu.surface(0).map(|(texture, _)| texture), Some(&first));
    gpu.surface_present(surface);
    let (shown, second_generation) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!(shown, &second);
    assert_ne!(second_generation, first_generation);
    assert_eq!(acquired(&mut gpu, surface), first);
    assert_eq!(gpu.take_presented(), vec![0, 0]);
    assert_eq!(gpu.take_error(), None);
}
