use super::*;
use crate::settings::{Settings, SettingsContent};
use crate::{LINUX_DESKTOP_EDITOR, WORKSPACE_EDITOR};
use uuid::Uuid;

#[test]
fn a_desktop_session_keeps_its_own_profile_apart_from_the_app() {
    let laptop = Uuid::new_v4();
    let (app, desktop) = (Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [Settings::add_profile(WORKSPACE_EDITOR, laptop, app)],
    );
    assert_eq!(
        settings.root().profile(LINUX_DESKTOP_EDITOR, laptop),
        None,
        "a session started as the desktop does not take the profile the app uses"
    );

    let both = edited(
        &settings,
        [Settings::add_profile(LINUX_DESKTOP_EDITOR, laptop, desktop)],
    );
    assert_eq!(both.root().profile(WORKSPACE_EDITOR, laptop), Some(app));
    assert_eq!(
        both.root().profile(LINUX_DESKTOP_EDITOR, laptop),
        Some(desktop)
    );
    assert_eq!(both.root().profiles().len(), 2);
}
