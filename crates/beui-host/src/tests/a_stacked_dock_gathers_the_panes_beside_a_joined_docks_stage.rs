use super::*;
use crate::dock::DockMode;

#[test]
fn a_stacked_dock_gathers_the_panes_beside_a_joined_docks_stage() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = staged_nested_dock();
    let mut harness = Harness::sized(document, PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        nested.outer.mode().get_untracked(),
        DockMode::Stacked,
        "the outer dock has no stage of its own, so it stacks"
    );

    nested.inner.show(&10);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let stage = harness.rect(harness.find("content.10"));
    let inspector = harness.rect(harness.find("content.11"));
    assert!(
        (stage.width() - PHONE_VIEWPORT.x).abs() < 1.0 && stage.bottom() <= inspector.top() + 1.0,
        "the editor shows its stage with its inspector gathered below it, {stage:?} {inspector:?}"
    );
    assert!(
        harness.document().find_test_id("content.1").is_none()
            || !harness.rect(harness.find("content.1")).is_positive(),
        "and the files are stacked behind it"
    );
    assert_eq!(nested.inner.shown(), Some(10));

    nested.outer.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let files = harness.rect(harness.find("content.1"));
    assert!((files.width() - PHONE_VIEWPORT.x).abs() < 1.0);
}
