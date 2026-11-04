use super::*;

#[test]
fn the_headless_platform_takes_its_window_and_screen() {
    assert_eq!(
        choose(
            &["winit"],
            with(&[
                (PLATFORM, "headless"),
                (HEADLESS_WINDOW, "800x600@2"),
                (HEADLESS_SCREEN, "1440x900"),
            ])
        ),
        Ok(Choice {
            platform: Base::Headless {
                window: WindowSize {
                    size: Vec2::new(800.0, 600.0),
                    scale: 2.0,
                },
                screen: Some(Vec2::new(1440.0, 900.0)),
                pinned_clock: false,
            },
            ..NATIVE
        })
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (HEADLESS_WINDOW, "big")])
        ),
        Err("BEUI_HEADLESS_WINDOW takes WIDTHxHEIGHT[@SCALE], not big".to_owned())
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[(PLATFORM, "headless"), (HEADLESS_SCREEN, "8000x8000")])
        ),
        Ok(Choice {
            platform: Base::Headless {
                window: WindowSize::parse(HEADLESS_WINDOW_SIZE).unwrap(),
                screen: Some(Vec2::new(8000.0, 8000.0)),
                pinned_clock: false,
            },
            ..NATIVE
        })
    );
    assert_eq!(
        choose(
            &["winit"],
            with(&[
                (PLATFORM, "headless"),
                (HEADLESS_WINDOW, "800x600@2"),
                (HEADLESS_SCREEN, "8000x8000"),
            ])
        ),
        Err("BEUI_HEADLESS_SCREEN at scale 2 is more than 8192 pixels across".to_owned())
    );
}
