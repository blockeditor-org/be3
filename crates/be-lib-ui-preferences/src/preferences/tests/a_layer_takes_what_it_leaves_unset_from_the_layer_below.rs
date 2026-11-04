use crate::{CanvasTouch, Motion, PreferenceLayer, Preferences, ThemeKind};

#[test]
fn a_layer_takes_what_it_leaves_unset_from_the_layer_below() {
    let system = PreferenceLayer {
        theme: Some(ThemeKind::Light),
        motion: Some(Motion::Instant),
        canvas_touch: None,
    };
    let app = PreferenceLayer {
        theme: Some(ThemeKind::EPaper),
        motion: None,
        canvas_touch: Some(CanvasTouch::Off),
    };
    assert_eq!(
        app.over(system).resolve(),
        Preferences {
            theme: ThemeKind::EPaper,
            motion: Motion::Instant,
            canvas_touch: CanvasTouch::Off,
        }
    );
    assert_eq!(PreferenceLayer::EMPTY.resolve(), Preferences::default());
}
