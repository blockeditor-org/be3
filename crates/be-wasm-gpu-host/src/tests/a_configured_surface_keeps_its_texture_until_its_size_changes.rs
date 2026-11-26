use super::web::*;
use super::*;

#[test]
fn a_configured_surface_keeps_its_texture_until_its_size_changes() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (320, 200), Vec::new());
    let (texture, generation) = gpu.surface(0).expect("the surface has a texture");
    assert_eq!((texture.width(), texture.height()), (320, 200));
    gpu.resize(context, 320, 200);
    configure(&mut gpu, device, context, Vec::new());
    assert_eq!(gpu.surface(0).map(|(_, generation)| generation), Some(generation));
    gpu.resize(context, 640, 400);
    let (texture, next) = gpu.surface(0).expect("the surface has a texture");
    assert_eq!((texture.width(), texture.height()), (640, 400));
    assert_ne!(next, generation);
    assert!(gpu.take_logged().is_empty());
}
