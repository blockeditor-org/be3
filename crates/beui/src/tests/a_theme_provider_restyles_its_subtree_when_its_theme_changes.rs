use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_theme_provider_restyles_its_subtree_when_its_theme_changes() {
    let _root = crate::reactive::RootScope::new();
    let theme = create_signal(Theme::DARK);
    let (document, [_button]) = toolbar_of(move || {
        [view! {
            <ThemeProvider theme>
                <Button label="Themed" variant=ButtonVariant::Primary />
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

    let dark = harness.frame(vec![]);
    assert_eq!(fills(&dark, Theme::DARK.accent), 1);
    assert_eq!(fills(&dark, Theme::EPAPER.accent), 0);

    theme.set(Theme::EPAPER);
    let epaper = harness.frame(vec![]);
    assert_eq!(fills(&epaper, Theme::DARK.accent), 0);
    assert_eq!(fills(&epaper, Theme::EPAPER.accent), 1);
    assert_eq!(
        Theme::of(harness.document().preferences().theme),
        Theme::DARK,
        "a provider restyles its subtree without changing the document theme"
    );
}
