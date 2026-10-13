use super::*;
use crate::WORKSPACE_EDITOR;
use crate::settings::{Settings, SettingsContent};

#[test]
fn settings_split_across_two_blocks_read_as_one() {
    let (laptop, phone) = (Uuid::new_v4(), Uuid::new_v4());
    let (first, second, session) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let kind = Uuid::new_v4();
    let kept = edited(
        &SettingsContent::default(),
        [
            Settings::add_profile(laptop, first),
            Settings::set_kind(kind, first),
        ],
    );
    let other = edited(
        &SettingsContent::default(),
        [
            Settings::add_profile(phone, second),
            Settings::add_session(WORKSPACE_EDITOR, phone, session),
            Settings::set_kind(kind, second),
        ],
    );
    let merged = Settings::merged([kept.root(), other.root()]);
    assert_eq!(merged.profile(laptop), Some(first));
    assert_eq!(merged.profile(phone), Some(second));
    assert_eq!(merged.session(WORKSPACE_EDITOR, phone), Some(session));
    assert_eq!(merged.kind(kind), Some(first), "the first block wins");
}
