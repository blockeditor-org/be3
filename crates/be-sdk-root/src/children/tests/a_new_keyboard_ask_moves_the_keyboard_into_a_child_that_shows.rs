use super::*;

#[test]
fn a_new_keyboard_ask_moves_the_keyboard_into_a_child_that_shows() {
    let mut children = placing(vec![up(1, ChildMode::Live), up(2, ChildMode::Live)]);
    ask(&mut children, 2, 1);
    assert_eq!(
        keyboard(&mut children, Some(PLACER), &[]),
        Some(KeyboardMove::Into(LayerId(202)))
    );
    // Acted on, the ask ends: the keyboard pressed back into the placer stays there.
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);
    // A placement that carries the same serial again is no new ask; a higher one is.
    ask(&mut children, 2, 1);
    assert_eq!(keyboard(&mut children, Some(PLACER), &[]), None);
    ask(&mut children, 1, 2);
    assert_eq!(
        keyboard(&mut children, Some(LayerId(202)), &[]),
        Some(KeyboardMove::Into(LayerId(201)))
    );
}
