use be_sdk_beui::beui::icons::{ICON_EYE, ICON_LOCK, ICON_LOG_IN};

use super::*;

#[test]
fn access_marker_marks_limited_access() {
    assert_eq!(access_marker(AccessLevel::Manage), None);
    assert_eq!(access_marker(AccessLevel::Edit), None);
    assert_eq!(access_marker(AccessLevel::View), Some(ICON_EYE));
    assert_eq!(access_marker(AccessLevel::Join), Some(ICON_LOG_IN));
    assert_eq!(access_marker(AccessLevel::KnowExists), Some(ICON_LOCK));
    assert_eq!(access_marker(AccessLevel::None), Some(ICON_LOCK));
}
