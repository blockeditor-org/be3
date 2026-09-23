use super::*;

#[test]
fn plugin_loading_rect_disappears_when_the_screen_is_presented() {
    let screen = ScreenId(7);
    let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 40.0));
    let mut layout = ScreenLayout::default();

    assert_eq!(plugin_loading_rect(&layout, screen, rect), Some(rect));

    layout.screens.push(ScreenPlacement {
        screen,
        instance: EditorInstanceId(1),
        region: EditorRegion::Frame,
        x: 0,
        y: 0,
        width: 30,
        height: 40,
        scale_factor_millis: 1000,
    });

    assert_eq!(plugin_loading_rect(&layout, screen, rect), None);
}
