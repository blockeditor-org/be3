use super::*;

#[test]
fn a_long_window_title_ends_inside_the_window_switcher() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let long = TestWindow {
        title: "A very long window title that keeps going well past the edge of the switcher"
            .to_owned(),
        ..host_window(1, false)
    };
    fixture.show_windows(&vec![long, host_window(2, true)]);
    fixture.settle();

    assert!(fixture.test.host_key(Modifiers::ALT, Key::Tab));
    fixture.settle();
    assert!(
        fixture
            .test
            .shown(&format!("dock.switch.{}", fixture.tab_of(1)))
    );
    fixture
        .test
        .snapshot("a_long_window_title_ends_inside_the_window_switcher");
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
}
