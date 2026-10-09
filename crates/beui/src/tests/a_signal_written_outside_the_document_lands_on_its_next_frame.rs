use super::*;
use crate::reactive::{create_signal, view};
use crate::styled::{Button, ButtonVariant, Theme, ThemeProvider};

#[test]
fn a_signal_written_outside_the_document_lands_on_its_next_frame() {
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
    assert_eq!(fills(&harness.frame(vec![]), Theme::DARK.accent), 1);

    set_theme.set(Theme::EINK);
    assert!(crate::reactive::zone_pending(harness.document().zone()));

    let eink = harness.frame(vec![]);
    assert_eq!(fills(&eink, Theme::DARK.accent), 0);
    assert_eq!(fills(&eink, Theme::EINK.accent), 1);
}
