use super::*;

#[test]
fn a_layout_gives_each_shown_screen_its_own_surface() {
    let screens = [
        screen(1, 1, 100, 200, 2.0),
        screen(2, 2, 0, 0, 1.0),
        region_screen(EditorRegion::Preview, 3, 1, 400, 30, 1.0),
    ];
    let layout = ScreenLayout::placed(&screens, |screen| screen.0 as u32 * 10);
    assert_eq!(
        layout.screens,
        vec![
            ScreenPlacement {
                screen: ScreenId(1),
                instance: EditorInstanceId(1),
                region: EditorRegion::Frame,
                surface: 10,
                width: 100,
                height: 200,
                scale_factor_millis: 2000,
            },
            ScreenPlacement {
                screen: ScreenId(3),
                instance: EditorInstanceId(1),
                region: EditorRegion::Preview,
                surface: 30,
                width: 400,
                height: 30,
                scale_factor_millis: 1000,
            },
        ]
    );
}
