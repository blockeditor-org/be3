use super::*;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn the_inspectors_theme_overrides_the_apps_until_it_is_cleared() {
    let (document, [_card]) = toolbar_of(|| {
        [view! {
            <Card>
                <Text style=TextStyle::CAPTION>"Themed"</Text>
            </Card>
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    let app = PreferenceLayer {
        theme: Some(ThemeKind::Light),
        ..PreferenceLayer::EMPTY
    };
    harness
        .context()
        .set_preference_layer(PreferenceSource::App, app);
    let captions = |output: &crate::native::FrameOutput, theme: Theme| {
        output
            .main().shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::native::Shape::Text { color, .. } if *color == theme.text_muted)
            })
            .count()
    };
    assert_eq!(captions(&harness.frame(vec![]), Theme::LIGHT), 1);

    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(vec![]);
    harness.choose_in_simulation("theme", Some(crate::ThemeKind::EPaper));
    let restyled = harness.frame(vec![]);
    assert_eq!(
        Theme::of(harness.document().preferences().theme),
        Theme::EPAPER
    );
    assert_eq!(captions(&restyled, Theme::EPAPER), 1);
    assert_eq!(
        harness.context().preference_layer(PreferenceSource::App),
        app,
        "the override is not the app's own preference"
    );

    harness.choose_in_simulation("theme", None::<crate::ThemeKind>);
    harness.frame(vec![]);
    assert_eq!(
        Theme::of(harness.document().preferences().theme),
        Theme::LIGHT
    );
}
