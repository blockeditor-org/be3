use super::*;

#[test]
fn a_tab_walks_back_and_forward_through_its_history() {
    let (mut fixture, opened) = editor();
    let second = Uuid::new_v4();

    show(&mut fixture, opened, None, None);
    show(&mut fixture, second, None, Some(opened));
    assert_eq!(fixture.focused(), Some(second));

    fixture.test.click("workspace.back");
    fixture.settle();
    assert_eq!(fixture.shown(), vec![opened]);
    assert_eq!(fixture.focused(), Some(opened));

    fixture.test.click("workspace.forward");
    fixture.settle();
    assert_eq!(fixture.shown(), vec![second]);
    assert_eq!(fixture.focused(), Some(second));
}
