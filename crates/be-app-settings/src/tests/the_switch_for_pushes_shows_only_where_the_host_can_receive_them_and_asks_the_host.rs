use super::*;
use be_sdk_beui::{PushDevice, PushDeviceWatch};

#[test]
fn the_switch_for_pushes_shows_only_where_the_host_can_receive_them_and_asks_the_host() {
    let mut settings = editor_as::<SettingsRoot>(Uuid::nil());
    settings.set_size(Vec2::new(420.0, 760.0));
    settings.run();
    assert!(
        !settings.shown("settings.push-device"),
        "a host that cannot receive pushes offers no switch"
    );

    settings.set_watched::<PushDeviceWatch>(&Some(PushDevice::Off));
    settings.run();
    assert!(settings.shown("settings.push-device"));
    settings.snapshot("push_switch_off");

    settings.reveal("settings.push-device");
    settings.click("settings.push-device");
    settings.settle();
    assert_eq!(settings.take_push_device_requests(), [true]);

    settings.set_watched::<PushDeviceWatch>(&Some(PushDevice::On));
    settings.run();
    settings.reveal("settings.push-device");
    settings.click("settings.push-device");
    settings.settle();
    assert_eq!(settings.take_push_device_requests(), [false]);

    settings.set_watched::<PushDeviceWatch>(&Some(PushDevice::Denied));
    settings.run();
    settings.reveal("settings.push-device");
    settings.click("settings.push-device");
    settings.settle();
    assert!(
        settings.take_push_device_requests().is_empty(),
        "a refused permission can only be undone in the browser"
    );
    settings.snapshot("push_switch_denied");
}
