use super::*;

#[test]
fn a_configured_surface_shows_a_copy_rather_than_the_texture_being_drawn() {
    let mut gpu = gpu();
    gpu.configure_surface(0, &configuration(320, 200));
    let handle = gpu.acquire_surface(0);
    let drawn = gpu
        .textures
        .get(handle, "texture")
        .expect("the acquired surface should be a texture")
        .clone();
    gpu.present_surface(0);
    let (shown, _) = gpu.surface(0).expect("the surface should have a texture");
    assert_ne!(&drawn, shown);
    assert_eq!((shown.width(), shown.height()), (320, 200));
    assert_eq!(gpu.take_presented(), vec![0]);
    assert_eq!(gpu.take_error(), None);
}
