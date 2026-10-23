use super::*;
use crate::reactive::*;
use std::cell::Cell;

#[test]
fn an_animation_restarted_from_its_step_keeps_its_new_schedule() {
    let _root = RootScope::new();
    let runs = create_signal(0);
    let finished = create_signal(0);
    let held = Rc::new(Cell::new(None::<Animation>));
    let saved = held.clone();
    let document = build_node(move || {
        let animation = create_animation(
            move |_| {
                runs.update(|n| *n += 1);
                if runs.get_untracked() == 1 {
                    let animation = saved.get().unwrap();
                    animation.stop();
                    animation.start();
                }
                false
            },
            move || finished.update(|n| *n += 1),
        );
        held.set(Some(animation));
        animation.start();
        view! {
            <Frame height=10.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(runs.get_untracked(), 2);
    assert_eq!(finished.get_untracked(), 1);
}
