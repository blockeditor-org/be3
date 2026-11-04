use super::*;

#[test]
fn a_moved_bar_leaves_no_focus_ring_behind_while_switching() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture
        .test
        .click(&format!("dock.bar.{}.grip", SHELL_BARS + 2));
    fixture.settle();
    for key in [Key::Escape, Key::Enter, Key::Enter, Key::Tab] {
        fixture.test.key(key, Modifiers::NONE);
        fixture.settle();
    }
    assert!(fixture.test.host_key(Modifiers::ALT, Key::Tab));
    fixture.settle();
    assert!(
        fixture
            .test
            .shown(&format!("dock.switch.{}", fixture.tab_of(1)))
    );
    fixture
        .test
        .snapshot("a_moved_bar_leaves_no_focus_ring_behind_while_switching");
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
}
