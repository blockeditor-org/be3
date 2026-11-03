use super::*;
use crate::dock::DockMode;

#[test]
fn a_narrow_dock_gathers_the_panes_beside_its_stage_below_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, layout) = staged_dock();
    let mut harness = Harness::sized(document, PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(layout.mode().get_untracked(), DockMode::Compact);

    let stage = harness.rect(harness.find("content.1"));
    let gathered = harness.rect(harness.find("dock.compact"));
    assert!(
        (stage.width() - PHONE_VIEWPORT.x).abs() < 1.0,
        "the stage takes the whole width, {stage:?}"
    );
    assert!(
        stage.bottom() <= gathered.top() + 1.0,
        "the gathered pane sits below the stage, {stage:?} {gathered:?}"
    );
    assert!(
        (gathered.height() - (PHONE_VIEWPORT.y * 0.4).round()).abs() < 2.0,
        "the gathered pane takes 40% of the height, {gathered:?}"
    );

    let tabs = [2, 3, 4].map(|tab| harness.rect(harness.find(&format!("dock.tab.{tab}"))));
    assert!(
        tabs.iter()
            .all(|tab| (tab.top() - tabs[0].top()).abs() < 1.0)
            && tabs[0].left() < tabs[1].left()
            && tabs[1].left() < tabs[2].left(),
        "every other pane's tabs share one strip, in the order they are declared: {tabs:?}"
    );
    assert!(gathered.contains_rect(harness.rect(harness.find("content.2"))));
    assert!(
        harness.document().find_test_id("content.4").is_none()
            || !harness.rect(harness.find("content.4")).is_positive(),
        "only the gathered pane's tab is shown"
    );

    harness.click(harness.center(harness.find("dock.tab.4")));
    harness.frame(Vec::new());
    assert!(gathered.contains_rect(harness.rect(harness.find("content.4"))));
}
