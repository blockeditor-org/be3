use crate::{Motion, PreferenceLayer, Preferences, ThemeKind};

#[test]
fn a_layer_takes_what_it_leaves_unset_from_the_layer_below() {
    let system = PreferenceLayer {
        theme: Some(ThemeKind::Light),
        motion: Some(Motion::Instant),
    };
    let app = PreferenceLayer {
        theme: Some(ThemeKind::EPaper),
        motion: None,
    };
    assert_eq!(
        app.over(system).resolve(),
        Preferences {
            theme: ThemeKind::EPaper,
            motion: Motion::Instant,
        }
    );
    assert_eq!(PreferenceLayer::EMPTY.resolve(), Preferences::default());
}
