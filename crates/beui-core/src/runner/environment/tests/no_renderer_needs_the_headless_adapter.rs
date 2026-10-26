use super::*;

#[test]
fn no_renderer_needs_the_headless_adapter() {
    assert_eq!(
        choose("winit", with(&[(ADAPTER, "headless"), (RENDERER, "none")]))
            .map(|choice| choice.render),
        Ok(false)
    );
    assert_eq!(
        choose("winit", with(&[(RENDERER, "none")])),
        Err(
            "BEUI_RENDERER=none needs the headless adapter: BEUI_ADAPTER=headless, \
             or BEUI_SPACE_ADAPTER=headless under BEUI_ADAPTER=space"
                .to_owned()
        )
    );
}
