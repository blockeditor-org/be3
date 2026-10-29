use super::*;
use crate::reactive::*;

#[test]
fn an_animation_can_start_its_next_phase_from_completion() {
    let _root = RootScope::new();
    let finished = create_signal(0);
    let document = build_node(move || {
        let next = create_animation(|_| false, move || finished.update(|n| *n += 1));
        let first = create_animation(
            |_| false,
            move || {
                finished.update(|n| *n += 1);
                next.start();
            },
        );
        first.start();
        view! {
            <Frame height=10.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(finished.get_untracked(), 2);
    harness.frame(Vec::new());
    assert_eq!(finished.get_untracked(), 2);
}
