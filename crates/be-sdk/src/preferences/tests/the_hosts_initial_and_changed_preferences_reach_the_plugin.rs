use super::*;

#[test]
fn the_hosts_initial_and_changed_preferences_reach_the_plugin() {
    let mut plugin = HeadlessPlugin::new("preferences", "Preferences", "1");
    let initial = Preferences {
        theme: ThemeKind::Light,
        motion: Motion::Instant,
        canvas_touch: Default::default(),
    };
    let accept = HelloAccepted {
        host_name: "test host".to_owned(),
        surface: None,
        preferences: initial,
    };
    plugin.accept(decode(&encode(&accept)).unwrap()).unwrap();
    assert_eq!(host_preferences(), initial);
    let changed = Preferences {
        theme: ThemeKind::EPaper,
        motion: Motion::Animated,
        canvas_touch: Default::default(),
    };
    plugin
        .set_preferences(decode(&encode(&changed)).unwrap())
        .unwrap();
    assert_eq!(host_preferences(), changed);
}
