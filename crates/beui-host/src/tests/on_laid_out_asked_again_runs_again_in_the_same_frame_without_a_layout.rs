use super::*;
use crate::primitives::*;

#[test]
fn on_laid_out_asked_again_runs_again_in_the_same_frame_without_a_layout() {
    let _root = crate::reactive::RootScope::new();
    let mut document = build_node(move || {
        view! {
            <Frame width=40.0 height=40.0 />
        }
    });
    let runs = Rc::new(std::cell::Cell::new(0));
    let counted = Rc::clone(&runs);
    document.on_laid_out(move || {
        counted.set(counted.get() + 1);
        if counted.get() < 3 {
            crate::native::with_document(crate::native::Document::run_laid_out_again);
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        runs.get(),
        3,
        "a hook that asks to run again is run again in the frame, until it stops asking"
    );
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "running it again lays nothing out that did not change"
    );
}
