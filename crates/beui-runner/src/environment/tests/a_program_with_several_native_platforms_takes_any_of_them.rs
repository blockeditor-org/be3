use super::*;

#[test]
fn a_program_with_several_native_platforms_takes_any_of_them() {
    let natives = ["winit", "drm"];
    assert_eq!(choose(&natives, with(&[])), Ok(NATIVE));
    assert_eq!(choose(&natives, with(&[(PLATFORM, "winit")])), Ok(NATIVE));
    assert_eq!(
        choose(&natives, with(&[(PLATFORM, "drm")])),
        Ok(Choice {
            platform: Base::Native(1),
            ..NATIVE
        })
    );
    assert_eq!(
        choose(&natives, with(&[(PLATFORM, "x11")])),
        Err("BEUI_PLATFORM takes winit, drm or headless, not x11".to_owned())
    );
}
