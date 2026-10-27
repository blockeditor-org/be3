use super::*;
use crate::native::with_reactive_scope;
use crate::reactive::*;
use crate::{PreferenceLayer, PreferenceSource};
use std::cell::Cell;

#[test]
fn animations_finish_once_and_keep_functional_timers() {
    let _root = RootScope::new();
    let finished = create_signal(0);
    let expired = create_signal(false);
    let held = Rc::new(Cell::new(None::<Animation>));
    let saved = held.clone();
    let document = build_node(move || {
        let animation = create_animation(|_| true, move || finished.update(|n| *n += 1));
        saved.set(Some(animation));
        animation.start();
        let timer = create_timer(move || {
            expired.set(true);
            None
        });
        timer.start(Duration::from_secs(1));
        view! {
            <Frame height=10.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(finished.get_untracked(), 0);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    assert_eq!(finished.get_untracked(), 1);
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().finish());
    assert_eq!(finished.get_untracked(), 1);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    assert!(expired.get_untracked());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Animated),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    assert!(!held.get().unwrap().running());
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().start());
    assert!(held.get().unwrap().running());
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().stop());
    assert_eq!(finished.get_untracked(), 1);
}
