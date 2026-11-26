use super::web::*;
use super::*;

#[test]
fn a_configured_surface_alternates_between_two_textures() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (320, 200), Vec::new());
    let first = current(&mut gpu, context);
    gpu.present(context);
    let (shown, first_generation) = gpu.surface(0).expect("the surface has a texture");
    assert_eq!(shown, &first);
    let second = current(&mut gpu, context);
    assert_ne!(second, first);
    assert_eq!(gpu.surface(0).map(|(texture, _)| texture), Some(&first));
    gpu.present(context);
    let (shown, second_generation) = gpu.surface(0).expect("the surface has a texture");
    assert_eq!(shown, &second);
    assert_ne!(second_generation, first_generation);
    assert_eq!(current(&mut gpu, context), first);
    assert_eq!(gpu.take_presented(), vec![0, 0]);
    assert!(gpu.take_logged().is_empty());
}
