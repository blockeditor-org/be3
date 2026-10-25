use super::*;

#[test]
fn space_wraps_the_native_adapter_unless_given_another() {
    let space = Choice {
        space: true,
        ..NATIVE
    };
    assert_eq!(
        choose("winit", with(&[(ADAPTER, "space")])),
        Ok(space.clone())
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(ADAPTER, "space"), (SPACE_ADAPTER, "winit")])
        ),
        Ok(space)
    );
    assert_eq!(
        choose(
            "winit",
            with(&[(ADAPTER, "space"), (SPACE_ADAPTER, "headless")])
        ),
        Ok(Choice {
            adapter: Base::Headless {
                window: WindowSize::parse(HEADLESS_WINDOW_SIZE).unwrap(),
                screen: None,
            },
            space: true,
            render: true,
        })
    );
}
