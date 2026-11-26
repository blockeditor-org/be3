use super::web::*;
use super::*;

#[test]
fn a_surface_shows_what_was_presented_until_it_is_presented_again() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (320, 200), Vec::new());
    assert!(
        gpu.presented_surface(0).is_none(),
        "a surface configured but never presented shows nothing"
    );
    gpu.present(context);
    let (texture, generation) = gpu
        .presented_surface(0)
        .map(|(texture, generation)| (texture.clone(), generation))
        .expect("the surface was presented");

    gpu.resize(context, 640, 400);
    let (kept, kept_generation) = gpu
        .presented_surface(0)
        .expect("resized, the surface still shows what was presented");
    assert_eq!(
        (kept, kept_generation),
        (&texture, generation),
        "a new size shows nothing new until it is presented"
    );
    assert_ne!(
        gpu.surface(0).map(|(live, _)| live),
        Some(&texture),
        "though the surface's own textures are new"
    );

    gpu.present(context);
    let (shown, _) = gpu.presented_surface(0).expect("the surface was presented");
    assert_eq!(shown.width(), 640, "presented again, it shows its new size");
    gpu.detach_surface(0);
    assert!(
        gpu.presented_surface(0).is_none(),
        "let go of, it shows nothing"
    );
}
