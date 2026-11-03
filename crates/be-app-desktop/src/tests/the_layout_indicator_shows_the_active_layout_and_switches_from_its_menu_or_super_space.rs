use super::*;

fn layouts(active: u32) -> KeyboardLayouts {
    let layout = |name: &str, label: &str| KeyboardLayout {
        name: name.to_owned(),
        label: label.to_owned(),
    };
    KeyboardLayouts {
        layouts: vec![layout("English (US)", "EN"), layout("German", "DE")],
        active,
    }
}

#[test]
fn the_layout_indicator_shows_the_active_layout_and_switches_from_its_menu_or_super_space() {
    let mut fixture = Fixture::with_windows(&[1]);
    let space = KeyChord::new(Key::Space, Modifiers::LOGO);
    fixture
        .test
        .set_watched::<KeyboardLayoutsWatch>(&KeyboardLayouts {
            layouts: layouts(0).layouts[..1].to_vec(),
            active: 0,
        });
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.layout"),
        "one layout has nothing to switch to"
    );
    assert!(!fixture.test.intercepted_keys().contains(&space));

    fixture
        .test
        .set_watched::<KeyboardLayoutsWatch>(&layouts(0));
    fixture.settle();
    assert!(fixture.test.shown("desktop.layout"));
    assert_eq!(text_of(&fixture, "desktop.layout").as_deref(), Some("EN"));
    fixture.test.snapshot("the_layout_indicator_in_the_bar");

    fixture.test.click("desktop.layout");
    fixture.settle();
    fixture.test.settle();
    assert!(fixture.test.shown("desktop.layout.0"));
    assert!(fixture.test.shown("desktop.layout.1"));
    assert!(fixture.says("English (US)") && fixture.says("German"));
    fixture.test.snapshot("the_layout_menu");

    fixture.test.click("desktop.layout.1");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::switch_layout),
        vec![1],
        "choosing a layout asks the host to switch to it"
    );

    fixture
        .test
        .set_watched::<KeyboardLayoutsWatch>(&layouts(1));
    fixture.settle();
    assert_eq!(
        text_of(&fixture, "desktop.layout").as_deref(),
        Some("DE"),
        "the indicator follows the layout the host reports"
    );

    assert!(
        fixture.test.intercepted_keys().contains(&space),
        "Super+Space is the desktop's, even from a program with the keyboard"
    );
    assert!(fixture.test.host_key(Modifiers::LOGO, Key::Space));
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::switch_layout),
        vec![0],
        "Super+Space switches to the next layout, past the last back to the first"
    );
}

fn text_of(fixture: &Fixture, test_id: &str) -> Option<String> {
    let document = fixture.test.document();
    let id = document.find_test_id(test_id)?;
    text_under(&document, id)
}
