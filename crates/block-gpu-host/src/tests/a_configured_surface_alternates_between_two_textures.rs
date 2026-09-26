use super::*;

fn acquired(gpu: &mut Gpu) -> wgpu::Texture {
    let handle = gpu.acquire_surface(0);
    gpu.textures
        .get(handle, "texture")
        .expect("the acquired surface should be a texture")
        .clone()
}

#[test]
fn a_configured_surface_alternates_between_two_textures() {
    let mut gpu = gpu();
    gpu.configure_surface(0, &configuration(320, 200));
    let first = acquired(&mut gpu);
    gpu.present_surface(0);
    let (shown, first_generation) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!(shown, &first);
    let second = acquired(&mut gpu);
    assert_ne!(second, first);
    assert_eq!(gpu.surface(0).map(|(texture, _)| texture), Some(&first));
    gpu.present_surface(0);
    let (shown, second_generation) = gpu.surface(0).expect("the surface should have a texture");
    assert_eq!(shown, &second);
    assert_ne!(second_generation, first_generation);
    assert_eq!(acquired(&mut gpu), first);
    assert_eq!(gpu.take_presented(), vec![0, 0]);
    assert_eq!(gpu.take_error(), None);
}
