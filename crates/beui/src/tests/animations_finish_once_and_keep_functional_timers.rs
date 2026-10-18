use super::*;
use crate::reactive::*;
use std::cell::Cell;

#[test]
fn animations_finish_once_and_keep_functional_timers() {
    let _root = RootScope::new();
    let (finished, set_finished) = create_signal(0);
    let (expired, set_expired) = create_signal(false);
    let held = Rc::new(Cell::new(None::<Animation>));
    let saved = held.clone();
    let document = build(move || {
        let animation = create_animation(|_| true, move || set_finished.update(|n| *n += 1));
        saved.set(Some(animation));
        animation.start();
        let timer = create_timer(move || {
            set_expired.set(true);
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
    harness.context.set_motion(Motion::Instant);
    harness.frame(Vec::new());
    assert_eq!(finished.get_untracked(), 1);
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().finish());
    assert_eq!(finished.get_untracked(), 1);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    assert!(expired.get_untracked());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    harness.context.set_motion(Motion::Animated);
    harness.frame(Vec::new());
    assert!(!held.get().unwrap().running());
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().start());
    assert!(held.get().unwrap().running());
    with_reactive_scope(harness.document_mut(), || held.get().unwrap().stop());
    assert_eq!(finished.get_untracked(), 1);
}
