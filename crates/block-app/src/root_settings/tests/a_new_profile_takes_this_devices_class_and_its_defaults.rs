use super::*;

const PATIENCE: Duration = Duration::from_secs(20);

fn arrived(block: Uuid) {
    block_app_stack::wait_for(PATIENCE, |shared| {
        shared.blocks.contains_key(&block).then_some(())
    })
    .expect("the new stack never held the block");
}

fn new_profile(class: DeviceClass) -> (RootSettings, Uuid) {
    let mut settings = RootSettings::new(WORKSPACE_EDITOR);
    let block = settings.ensure().expect("the settings are made");
    block_app_stack::hold(block, SettingsContent::CONTENT_TYPE);
    arrived(block);
    let profile = settings
        .ensure_profile(Uuid::new_v4(), class)
        .expect("a profile is made");
    block_app_stack::hold(profile, SettingsProfileContent::CONTENT_TYPE);
    arrived(profile);
    (settings, profile)
}

#[test]
fn a_new_profile_takes_this_devices_class_and_its_defaults() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    block_app_stack::wait_for(PATIENCE, |shared| {
        shared.graph.ready(block_app_stack::Query::Roots).then_some(())
    })
    .expect("the graph never loaded");

    let (mut settings, profile) = new_profile(DeviceClass::EpaperTablet);
    let held = decode::<SettingsProfileContent>(profile)
        .expect("the profile has content")
        .root();
    assert_eq!(held.class, Some(DeviceClass::EpaperTablet));
    assert_eq!(
        RootSettings::named(profile, "Profile"),
        DeviceClass::EpaperTablet.name()
    );
    let display = held
        .kind(DisplaySettingsContent::CONTENT_TYPE)
        .expect("the profile keeps display settings of its own");
    block_app_stack::hold(display, DisplaySettingsContent::CONTENT_TYPE);
    arrived(display);
    let kept = decode::<DisplaySettingsContent>(display)
        .expect("the display settings have content")
        .root();
    assert_eq!(kept, DeviceClass::EpaperTablet.display_settings());
    assert_eq!(
        settings.take_set_up(),
        Some(profile),
        "the host opens it to set up"
    );
    assert_eq!(settings.take_set_up(), None, "once");

    let (mut settings, profile) = new_profile(DeviceClass::Desktop);
    let held = decode::<SettingsProfileContent>(profile)
        .expect("the profile has content")
        .root();
    assert_eq!(held.class, Some(DeviceClass::Desktop));
    assert!(held.kinds.is_empty(), "a desktop keeps nothing of its own");
    assert_eq!(settings.take_set_up(), None, "nothing to set up");
}
