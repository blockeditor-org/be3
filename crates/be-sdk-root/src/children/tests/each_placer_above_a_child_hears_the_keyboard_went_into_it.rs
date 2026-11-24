use super::*;

#[test]
fn each_placer_above_a_child_hears_the_keyboard_went_into_it() {
    // App 5's editor 1 places child 1, whose app 9's editor 4 places child 7.
    let outer = up(1, ChildMode::Live);
    let mut inner = up(7, ChildMode::Live);
    inner.editor = EditorInstanceId(4);
    *outer.shared.nested.borrow_mut() = placing(vec![inner]);
    outer.shared.nested.borrow_mut().instance = 9;
    let children = placing(vec![outer, up(2, ChildMode::Live)]);

    assert_eq!(
        children.focused_into(LayerId(207)),
        vec![
            (9, EditorInstanceId(4), ChildId(7)),
            (5, EditorInstanceId(1), ChildId(1)),
        ],
        "from the innermost placer out"
    );
    assert_eq!(
        children.focused_into(LayerId(202)),
        vec![(5, EditorInstanceId(1), ChildId(2))]
    );
    assert!(children.focused_into(PLACER).is_empty());
    assert!(children.focused_into(LayerId(1)).is_empty());
}
