use super::*;
use crate::settings::{Settings, SettingsContent};
use crate::{ChildChange, DESKTOP_EDITOR, Root, WORKSPACE_EDITOR};

#[test]
fn a_device_uses_the_settings_profile_and_session_it_picked() {
    let (laptop, phone) = (Uuid::new_v4(), Uuid::new_v4());
    let (desk, travel, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (app, desktop) = (Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [
            Settings::add_profile(laptop, desk),
            Settings::add_profile(phone, travel),
            Settings::add_session(WORKSPACE_EDITOR, laptop, app),
        ],
    );
    assert_eq!(settings.root().profile(laptop), Some(desk));
    assert_eq!(settings.root().profile(phone), Some(travel));
    assert_eq!(settings.root().profile(Uuid::new_v4()), None);
    assert_eq!(settings.root().session(WORKSPACE_EDITOR, laptop), Some(app));
    assert_eq!(
        settings.root().session(DESKTOP_EDITOR, laptop),
        None,
        "a device started as the desktop does not take the session the app uses"
    );
    assert_eq!(settings.root().references().len(), 3);

    let settings = edited(
        &settings,
        [
            Settings::add_session(DESKTOP_EDITOR, laptop, desktop),
            Settings::use_profile(phone, desk),
        ],
    );
    assert_eq!(settings.root().session(WORKSPACE_EDITOR, laptop), Some(app));
    assert_eq!(
        settings.root().session(DESKTOP_EDITOR, laptop),
        Some(desktop)
    );
    assert_eq!(settings.root().profile(phone), Some(desk));
    assert_eq!(
        settings.root().profiles().len(),
        2,
        "the other profile stays"
    );

    let replaced = edited(
        &settings,
        settings.root().child_edit(ChildChange::Replace {
            old: desk,
            new: copy,
        }),
    );
    assert_eq!(replaced.root().profile(laptop), Some(copy));
    assert!(!replaced.root().profiles().contains(&desk));

    let deleted = edited(
        &replaced,
        replaced.root().child_edit(ChildChange::Delete(app)),
    );
    assert_eq!(deleted.root().session(WORKSPACE_EDITOR, laptop), None);
    assert_eq!(deleted.root().sessions(), [desktop]);
}
