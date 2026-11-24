use super::*;

#[test]
fn a_keyboard_ask_for_a_child_not_up_takes_the_keyboard_out_and_gives_it_once_up() {
    let mut starting = up(1, ChildMode::Live);
    starting.up = false;
    let mut children = placing(vec![starting]);
    ask(&mut children, 1, 1);
    // Nothing waits for the child's app: the keyboard leaves the placer at the settle that
    // carries the ask, so keys typed meanwhile reach nothing.
    assert_eq!(
        keyboard(&mut children, Some(PLACER), &[]),
        Some(KeyboardMove::Out)
    );
    assert_eq!(keyboard(&mut children, None, &[]), None);
    // Up, but not drawn yet: it still waits.
    children.shown[0].up = true;
    assert_eq!(keyboard(&mut children, None, &[LayerId(201)]), None);
    assert_eq!(
        keyboard(&mut children, None, &[]),
        Some(KeyboardMove::Into(LayerId(201)))
    );
    assert_eq!(keyboard(&mut children, None, &[]), None);

    // A keyboard in another of the editor's children stays there until the child is up.
    let mut starting = up(1, ChildMode::Live);
    starting.up = false;
    let mut children = placing(vec![starting, up(2, ChildMode::Live)]);
    ask(&mut children, 1, 1);
    assert_eq!(keyboard(&mut children, Some(LayerId(202)), &[]), None);
    children.shown[0].up = true;
    assert_eq!(
        keyboard(&mut children, Some(LayerId(202)), &[]),
        Some(KeyboardMove::Into(LayerId(201)))
    );
}
