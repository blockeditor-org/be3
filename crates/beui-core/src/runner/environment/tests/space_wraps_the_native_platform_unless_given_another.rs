use super::*;

#[test]
fn space_wraps_the_native_platform_unless_given_another() {
    let space = Choice {
        space: true,
        ..NATIVE
    };
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "space")])),
        Ok(space.clone())
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(PLATFORM, "space"), (SPACE_PLATFORM, "winit")])
        ),
        Ok(space)
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(PLATFORM, "space"), (SPACE_PLATFORM, "headless")])
        ),
        Ok(Choice {
            platform: Base::Headless {
                window: WindowSize::parse(HEADLESS_WINDOW_SIZE).unwrap(),
                screen: None,
            },
            space: true,
            render: true,
        })
    );
}
