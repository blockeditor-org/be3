use super::*;
use crate::dock::unstyled::DockMode;

#[test]
fn a_stacked_dock_shows_a_nested_docks_tabs_as_its_own() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(nested.outer.mode().get_untracked(), DockMode::Stacked);
    assert_eq!(
        nested.inner.mode().get_untracked(),
        DockMode::Stacked,
        "the inner layout reads the outer dock's mode"
    );

    nested.inner.show(&11);
    harness.frame(Vec::new());
    assert_eq!(nested.inner.shown(), Some(11));
    let panel = harness.rect(harness.find("content.11"));
    assert!(
        panel.width() > VIEWPORT.x - 1.0,
        "the inner tab fills the stacked dock, {panel:?}"
    );

    nested.outer.show(&1);
    harness.frame(Vec::new());
    assert_eq!(nested.outer.shown(), Some(1));
    assert_eq!(nested.inner.shown(), None);
}
