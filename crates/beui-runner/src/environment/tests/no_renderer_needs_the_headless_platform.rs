use super::*;

#[test]
fn no_renderer_needs_the_headless_platform() {
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (RENDERER, "none")])
        )
        .map(|choice| choice.render),
        Ok(false)
    );
    assert_eq!(
        choose(&["winit"], with(&[(RENDERER, "none")])),
        Err("BEUI_RENDERER=none needs the headless platform: BEUI_PLATFORM=headless".to_owned())
    );
}
