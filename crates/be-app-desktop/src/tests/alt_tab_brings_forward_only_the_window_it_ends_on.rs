use super::*;

#[test]
fn alt_tab_brings_forward_only_the_window_it_ends_on() {
    let mut fixture = Fixture::with_windows(&[1, 2, 3]);
    let alt = Modifiers::ALT;

    assert!(fixture.test.host_key(alt, Key::Tab));
    fixture.settle();
    assert!(fixture.test.host_key(alt, Key::Tab));
    fixture.settle();
    assert!(
        fixture.focused_windows().is_empty(),
        "stepping past a window neither focuses nor raises it"
    );
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(fixture.focused_windows(), [HostWindowId(1)]);

    let windows = [1, 2, 3].map(|id| host_window(id, id == 1)).to_vec();
    fixture.show_windows(&windows);
    fixture.settle();
    fixture.test.clear_calls();
    assert!(fixture.test.host_key(alt, Key::Tab));
    fixture.settle();
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(3)],
        "the window passed over stays behind the one that was in front before the switch"
    );
}
