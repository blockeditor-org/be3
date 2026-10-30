use super::*;
use beui::dock::styled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_standalone_docks_layout_is_kept_on_this_device_and_every_instance_follows_it() {
    let (mut first, display, layout) = standalone(DisplaySettingsContent::default(), false);
    layout.place_bar(&3, DockBarPlacement::Beside(DockEdge::Left));
    first.run();
    let (key, saved) = kept_dock(&first, display);
    assert!(key.starts_with(&STANDALONE_TYPE.simple().to_string()));

    let kept = first.content::<DisplaySettingsContent>(Some(display));
    let (mut reopened, held, reopened_layout) = standalone(kept, false);
    assert_eq!(
        reopened_layout.bar_placement(&3),
        Some(DockBarPlacement::Beside(DockEdge::Left)),
        "a moved bar survives reopening"
    );

    layout.place_bar(
        &3,
        DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center),
    );
    first.run();
    let (_, changed) = kept_dock(&first, display);
    assert_ne!(changed, saved);
    reopened.edit::<DisplaySettingsContent>(
        Some(held),
        &DisplaySettings::set_dock_layout(&key, Some(changed)),
    );
    assert_eq!(
        reopened_layout.bar_placement(&3),
        Some(DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)),
        "an open instance follows the layout another one kept"
    );
}
