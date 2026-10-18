use super::*;
use crate::reactive::*;
use std::cell::Cell;

#[test]
fn a_disposed_animation_cancels_without_completion() {
    let _root = RootScope::new();
    let (shown, set_shown) = create_signal(true);
    let (finished, set_finished) = create_signal(false);
    let held = Rc::new(Cell::new(None::<Animation>));
    let saved = held.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Show condition={shown}>|| {
                    let animation = create_animation(|_| true, move || set_finished.set(true));
                    saved.set(Some(animation));
                    animation.start();
                    view! {
                        <Frame height=10.0 />
                    }
                }</Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), || set_shown.set(false));
    harness.frame(Vec::new());
    assert!(!held.get().unwrap().running());
    held.get().unwrap().finish();
    assert!(!finished.get_untracked());
}
