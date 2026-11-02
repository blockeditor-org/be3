use super::*;
use crate::dock::unstyled::DockMode;

#[test]
fn a_short_wide_dock_gathers_the_panes_beside_its_stage_to_its_right() {
    let _root = crate::reactive::RootScope::new();
    let (document, layout) = staged_dock();
    let mut harness = Harness::sized(document, LANDSCAPE_PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        layout.mode().get_untracked(),
        DockMode::Compact,
        "a short dock is compact however wide it is"
    );

    let stage = harness.rect(harness.find("content.1"));
    let gathered = harness.rect(harness.find("dock.compact"));
    assert!(
        stage.right() <= gathered.left() + 1.0,
        "the gathered pane sits right of the stage, {stage:?} {gathered:?}"
    );
    assert!(
        (gathered.width() - STAGED_SIDE_WIDTH).abs() < 2.0,
        "it is as wide as the dock's fixed side pane, {gathered:?}"
    );
    assert!(
        (gathered.height() - LANDSCAPE_PHONE_VIEWPORT.y).abs() < 1.0,
        "and as tall as the dock, {gathered:?}"
    );
    let tabs = [2, 3, 4].map(|tab| harness.rect(harness.find(&format!("dock.tab.{tab}"))));
    assert!(tabs.iter().all(|tab| (tab.top() - tabs[0].top()).abs() < 1.0));
}
