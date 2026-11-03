use super::*;
use crate::reactive::*;
use crate::styled::Frame;
use crate::styled::*;

#[test]
fn an_off_switch_shows_its_track_on_a_raised_surface_in_every_theme() {
    let _root = crate::reactive::RootScope::new();
    let theme = create_signal(Theme::DARK);
    let (document, [_switch]) = toolbar_of(move || {
        [view! {
            <ThemeProvider theme>
                <Frame fill=ColorRole::SurfaceRaised padding=Padding::all(Space::Lg)>
                    <Switch on=false />
                </Frame>
            </ThemeProvider>
        }]
    });
    let mut harness = Harness::new(document);
    let fills = |output: &crate::native::FrameOutput, fill: Color32| {
        output
            .main()
            .shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::native::Shape::Rect { color, stroke_width, .. }
                    if *color == fill && *stroke_width == 0.0)
            })
            .count()
    };

    for each in [Theme::DARK, Theme::OLED, Theme::LIGHT, Theme::EPAPER] {
        theme.set(each);
        let output = harness.frame(vec![]);
        assert_eq!(
            fills(&output, each.track),
            1,
            "{:?}: the off track is painted in the track colour",
            each.kind
        );
        for surface in [each.background, each.surface, each.surface_raised] {
            assert_ne!(
                each.track, surface,
                "{:?}: the off track stands out from every surface",
                each.kind
            );
        }
    }
}
