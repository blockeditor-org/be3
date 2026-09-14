use super::*;
use crate::reactive::{create_signal, view, with_reactive_scope};
use crate::styled::{Button, ButtonVariant, Theme, ThemeProvider};

#[test]
fn a_theme_provider_restyles_its_subtree_when_its_theme_changes() {
    let (theme, set_theme) = create_signal(Theme::DARK);
    let (document, [_button]) = toolbar_of(move || {
        [view! {
            <ThemeProvider theme>
                <Button label="Themed" variant=ButtonVariant::Primary />
            </ThemeProvider>
        }]
    });
    let mut harness = Harness::new(document);
    let fills = |output: &crate::FrameOutput, fill: Color32| {
        output
            .shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::Shape::Rect { color, stroke_width, .. }
                    if *color == fill && *stroke_width == 0.0)
            })
            .count()
    };

    let dark = harness.frame(vec![]);
    assert_eq!(fills(&dark, Theme::DARK.accent), 1);
    assert_eq!(fills(&dark, Theme::EINK.accent), 0);

    with_reactive_scope(harness.document_mut(), move || set_theme.set(Theme::EINK));
    let eink = harness.frame(vec![]);
    assert_eq!(fills(&eink, Theme::DARK.accent), 0);
    assert_eq!(fills(&eink, Theme::EINK.accent), 1);
    assert_eq!(
        harness.document().theme(),
        Theme::DARK,
        "a provider restyles its subtree without changing the document theme"
    );
}
