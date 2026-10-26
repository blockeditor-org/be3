use super::*;

#[test]
fn layers_stack_bottom_first_over_any_platform() {
    assert_eq!(
        choose("winit", with(&[(LAYERS, "space")])),
        Ok(Choice {
            layers: vec![Stacked::Space],
            ..NATIVE
        })
    );
    assert_eq!(
        choose("winit", with(&[(LAYERS, "space, space")])),
        Ok(Choice {
            layers: vec![Stacked::Space, Stacked::Space],
            ..NATIVE
        })
    );
    assert_eq!(choose("winit", with(&[(LAYERS, "")])), Ok(NATIVE));
    assert_eq!(
        choose("winit", with(&[(PLATFORM, "headless"), (LAYERS, "space")])),
        Ok(Choice {
            platform: Base::Headless {
                window: WindowSize::parse(HEADLESS_WINDOW_SIZE).unwrap(),
                screen: None,
            },
            layers: vec![Stacked::Space],
            render: true,
        })
    );
}
