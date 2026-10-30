use super::*;

#[test]
fn the_desktop_away_from_linux_hides_what_its_host_does_not_offer() {
    let mut fixture = Fixture::away_from_linux();
    fixture.settle();
    fixture.test.settle();

    for shown in [
        "desktop.bar.apps",
        "desktop.bar.windows",
        "desktop.bar.status",
        "desktop.apps",
        "desktop.clock",
        "desktop.status",
    ] {
        assert!(fixture.test.shown(shown), "the bar still shows {shown}");
    }
    for hidden in [
        "desktop.status.wifi",
        "desktop.status.volume",
        "desktop.status.power",
    ] {
        assert!(
            !fixture.test.shown(hidden),
            "a host with nothing to back {hidden} gets no control for it"
        );
    }
    fixture.test.snapshot("the_desktop_away_from_linux");
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.apps.sessions.new") && fixture.test.shown("desktop.apps.menu"),
        "the Apps panel still offers sessions and the menu"
    );
    assert!(
        !fixture.test.shown("desktop.apps.search"),
        "but no programs to search"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    let intercepted = fixture.test.intercepted_keys();
    assert!(
        !intercepted.contains(&KeyChord::tap(Key::Logo)),
        "Super is left alone when there are no programs to launch"
    );
    for (key, _) in BINDINGS {
        assert!(
            !intercepted.contains(&KeyChord::new(key, Modifiers::NONE)),
            "{key:?} is left to the system when the host has no media to control"
        );
    }
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::program)
            .is_empty()
    );
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::media)
            .is_empty()
    );

    fixture.test.set_watched::<ProgramsWatch>(&programs());
    fixture.test.set_watched::<PowerWatch>(&EVERYTHING);
    fixture.settle();
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.apps.search") && fixture.test.shown("desktop.status.power"),
        "what the host offers later shows up"
    );
}
