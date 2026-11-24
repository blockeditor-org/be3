use super::*;

#[test]
fn a_keyboard_ask_never_takes_the_keyboard_from_outside_its_editor() {
    // The root's own dialog, say, has the keyboard: the ask ends without moving it.
    let mut children = placing(vec![up(1, ChildMode::Live)]);
    ask(&mut children, 1, 1);
    assert_eq!(keyboard(&mut children, Some(LayerId(9)), &[]), None);
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);

    // Nor once it waits for its child: something else took the keyboard meanwhile.
    let mut starting = up(1, ChildMode::Live);
    starting.up = false;
    let mut children = placing(vec![starting]);
    ask(&mut children, 1, 1);
    assert_eq!(
        keyboard(&mut children, Some(PLACER), &[]),
        Some(KeyboardMove::Out)
    );
    children.shown[0].up = true;
    assert_eq!(keyboard(&mut children, Some(LayerId(9)), &[]), None);
    assert_eq!(keyboard(&mut children, None, &[]), None);
}
