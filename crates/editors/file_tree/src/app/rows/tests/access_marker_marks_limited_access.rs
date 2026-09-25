use block_editor_plugin::beui::icons::{ICON_LOCK, ICON_VISIBILITY};

use super::*;

#[test]
fn access_marker_marks_limited_access() {
    assert_eq!(access_marker(AccessLevel::Edit), None);
    assert_eq!(access_marker(AccessLevel::View), Some(ICON_VISIBILITY));
    assert_eq!(access_marker(AccessLevel::KnowExists), Some(ICON_LOCK));
    assert_eq!(access_marker(AccessLevel::None), Some(ICON_LOCK));
}
