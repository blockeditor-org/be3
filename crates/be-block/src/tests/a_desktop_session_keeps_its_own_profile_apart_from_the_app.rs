use super::*;
use crate::settings::{Settings, SettingsContent};
use uuid::Uuid;

#[test]
fn a_desktop_session_keeps_its_own_profile_apart_from_the_app() {
    let laptop = Uuid::new_v4();
    let (app, desktop) = (Uuid::new_v4(), Uuid::new_v4());
    let settings = edited(
        &SettingsContent::default(),
        [Settings::add_profile(laptop, false, app)],
    );
    assert_eq!(
        settings.root().profile(laptop, true),
        None,
        "a session started as the desktop does not take the profile the app uses"
    );

    let both = edited(&settings, [Settings::add_profile(laptop, true, desktop)]);
    assert_eq!(both.root().profile(laptop, false), Some(app));
    assert_eq!(both.root().profile(laptop, true), Some(desktop));
    assert_eq!(both.root().profiles().len(), 2);
}
