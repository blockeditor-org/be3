use super::*;

#[test]
fn power_actions_the_host_cannot_take_say_why() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.allow_power(PowerAvailability {
        lock: true,
        ..PowerAvailability::default()
    });
    fixture.hear(0.45, false, SPEAKERS);
    fixture.test.click("desktop.status");
    fixture.settle();

    let why = |fixture: &Fixture, key: &str| {
        fixture
            .test
            .accessible_description(&format!("desktop.power.{key}"))
    };
    assert_eq!(why(&fixture, "lock"), None, "locking is offered");
    for key in ["suspend", "log-out", "restart", "power-off"] {
        assert_eq!(
            why(&fixture, key).as_deref(),
            Some("Only when be3 is your login session"),
            "{key} needs the host to run the login session"
        );
    }

    fixture.test.snapshot("quick_settings_in_a_window");

    fixture.test.set_preferences(be_sdk_beui::Preferences {
        theme: be_sdk_beui::ThemeKind::Light,
        ..Default::default()
    });
    fixture.settle();
    fixture.test.snapshot("quick_settings_in_a_window_light");

    fixture.allow_power(PowerAvailability {
        suspend: false,
        ..EVERYTHING
    });
    assert_eq!(
        why(&fixture, "suspend").as_deref(),
        Some("This computer does not allow it")
    );
    assert_eq!(why(&fixture, "restart"), None);

    fixture.allow_power(PowerAvailability {
        log_out: false,
        ending: true,
        ..EVERYTHING
    });
    assert_eq!(
        why(&fixture, "log-out").as_deref(),
        Some("The session is ending")
    );
}
