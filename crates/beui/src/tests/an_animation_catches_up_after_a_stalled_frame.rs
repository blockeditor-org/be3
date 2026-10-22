use super::*;
use crate::reactive::*;

#[test]
fn an_animation_catches_up_after_a_stalled_frame() {
    let _root = RootScope::new();
    let progress = create_signal(0.0_f32);
    let document = build_node(move || {
        let animation = create_animation(
            move |frame| {
                let eased = tween(frame.elapsed.as_secs_f32(), 0.3, Easing::OutCubic);
                progress.set(eased);
                eased < 1.0
            },
            move || progress.set(1.0),
        );
        animation.start();
        view! {
            <Frame height=10.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    assert_eq!(progress.get_untracked(), 1.0);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
}
