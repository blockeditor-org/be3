use super::*;
use crate::reactive::*;
use crate::styled::Progress;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn an_instant_indeterminate_progress_stops_and_resumes_with_the_preference() {
    let document = build_node(move || {
        view! {
            <Progress />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Animated),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
}
