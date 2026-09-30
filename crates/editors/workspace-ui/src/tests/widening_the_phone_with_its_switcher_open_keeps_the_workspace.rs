use super::*;

#[test]
fn widening_the_phone_with_its_switcher_open_keeps_the_workspace() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    show(&mut fixture, opened, None);
    fixture.test.click("dock.switch");
    fixture.settle();
    assert!(fixture.test.shown("dock.switcher.home"));

    fixture.test.set_size(Vec2::new(1200.0, 800.0));
    fixture.settle();
    assert!(
        !fixture.test.shown("dock.switcher.home"),
        "the switcher goes with the phone's stacked screen"
    );
    assert_eq!(fixture.shown(), vec![opened], "the block stays on show");
}
