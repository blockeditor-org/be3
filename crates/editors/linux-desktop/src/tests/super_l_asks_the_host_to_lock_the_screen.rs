use super::*;

#[test]
fn super_l_asks_the_host_to_lock_the_screen() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.allow_power(EVERYTHING);
    assert!(
        fixture.test.intercepted_keys().contains(&KeyChord {
            key: Key::L,
            modifiers: Modifiers::LOGO,
        }),
        "the desktop takes Super+L before the program that has the keyboard"
    );
    assert!(fixture.test.app_key(Modifiers::LOGO, Key::L));
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<PowerAction>(),
        vec![PowerAction::Lock]
    );

    fixture.allow_power(PowerAvailability {
        lock: false,
        ..EVERYTHING
    });
    fixture.test.app_key(Modifiers::LOGO, Key::L);
    fixture.settle();
    assert!(
        fixture.test.take_actions::<PowerAction>().is_empty(),
        "a screen that is already locked is not locked again"
    );
}
