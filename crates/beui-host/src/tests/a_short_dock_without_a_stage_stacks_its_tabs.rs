use super::*;
use crate::dock::DockMode;

#[test]
fn a_short_dock_without_a_stage_stacks_its_tabs() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, LANDSCAPE_PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(nested.outer.mode().get_untracked(), DockMode::Stacked);

    harness.resize(WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(nested.outer.mode().get_untracked(), DockMode::Tiled);
}
