use super::*;
use crate::reactive::view;
use crate::styled::{Caption, Card, Theme};

#[test]
fn choosing_the_e_ink_theme_in_the_inspector_restyles_the_document() {
    let (document, [_card]) = toolbar_of(|| {
        [view! {
            <Card>
                <Caption content="Themed" />
            </Card>
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let captions = |output: &crate::FrameOutput| {
        output
            .shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::Shape::Text { color, .. } if *color == Theme::EINK.text_muted)
            })
            .count()
    };

    assert_eq!(captions(&harness.frame(vec![])), 0);

    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(vec![]);
    harness.click(harness.theme_option_center(1));
    let restyled = harness.frame(vec![]);
    assert_eq!(harness.document().theme(), Theme::EINK);
    assert_eq!(captions(&restyled), 1);

    harness.toggle_inspector();
    assert_eq!(captions(&harness.frame(vec![])), 1);
    assert_eq!(harness.document().theme(), Theme::EINK);
}
