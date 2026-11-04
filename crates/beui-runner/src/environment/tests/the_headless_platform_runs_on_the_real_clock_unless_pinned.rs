use super::*;

fn headless(pinned_clock: bool) -> Result<Choice, String> {
    Ok(Choice {
        platform: Base::Headless {
            window: WindowSize::parse(HEADLESS_WINDOW_SIZE).unwrap(),
            screen: None,
            pinned_clock,
        },
        ..NATIVE
    })
}

#[test]
fn the_headless_platform_runs_on_the_real_clock_unless_pinned() {
    assert_eq!(
        choose(&["winit"], with(&[(PLATFORM, "headless")])),
        headless(false)
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (HEADLESS_CLOCK, "real")])
        ),
        headless(false)
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (HEADLESS_CLOCK, "pinned")])
        ),
        headless(true)
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (HEADLESS_CLOCK, "stopped")])
        ),
        Err("BEUI_HEADLESS_CLOCK takes real or pinned, not stopped".to_owned())
    );
}
