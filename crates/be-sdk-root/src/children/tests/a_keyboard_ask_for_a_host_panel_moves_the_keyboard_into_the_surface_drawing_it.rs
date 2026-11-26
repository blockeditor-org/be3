use super::*;

/// A dock that gives a host panel's tab the keyboard asks for it as for any child that takes
/// input: the root moves it into the surface it draws the panel in once that is drawn, from a
/// sibling child of the same placer too (the keyboard a closing menu gave back to the Files), and
/// says the panel's placer has it there, so its dock shows that tab focused.
#[test]
fn a_keyboard_ask_for_a_host_panel_moves_the_keyboard_into_the_surface_drawing_it() {
    let mut children = placing(vec![up(1, ChildMode::Live)]);
    children.panels.push(panel(true));
    ask(&mut children, 9, 1);
    // Not drawn yet: the ask waits, and the keyboard stays in the sibling child.
    assert_eq!(
        keyboard(&mut children, Some(LayerId(201)), &[LayerId(300)]),
        None
    );
    assert_eq!(
        keyboard(&mut children, Some(LayerId(201)), &[]),
        Some(KeyboardMove::Into(LayerId(300)))
    );
    // Acted on, the ask ends.
    assert_eq!(keyboard(&mut children, Some(LayerId(300)), &[]), None);
    assert_eq!(
        children.focused_into(LayerId(300)),
        vec![(5, EditorInstanceId(1), ChildId(9))]
    );
}
