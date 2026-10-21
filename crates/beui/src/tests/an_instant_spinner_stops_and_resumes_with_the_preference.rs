use super::*;
use crate::reactive::*;
use crate::styled::Spinner;

#[test]
fn an_instant_spinner_stops_and_resumes_with_the_preference() {
    let document = build_node(move || {
        view! {
            <Spinner />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    harness.context.set_motion(Motion::Instant);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    harness.context.set_motion(Motion::Animated);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
}
