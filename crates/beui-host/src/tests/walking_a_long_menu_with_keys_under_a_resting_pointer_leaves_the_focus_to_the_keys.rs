use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn walking_a_long_menu_with_keys_under_a_resting_pointer_leaves_the_focus_to_the_keys() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <ForEach keys={(0..40).collect::<Vec<usize>>()}>|index: usize| {
                let label = format!("Item {index}");
                view! {
                    <unstyled::MenuItem label />
                }
            }</ForEach>
        };
        [view! {
            <ContextMenu items>
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let region = region.get();
    let mut harness = Harness::sized(document, vec2(400.0, 300.0));
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = |harness: &Harness, index: usize| {
        harness
            .document()
            .api::<unstyled::MenuListApi>(content)
            .row_button(index)
    };
    let focused = |harness: &Harness, index: usize| {
        harness
            .document()
            .api::<unstyled::ButtonApi>(row(harness, index))
            .focused()
            .get()
    };
    let under = |harness: &Harness, pos: Pos2| {
        (0..40).find(|index| harness.rect(row(harness, *index)).contains(pos))
    };

    let rest = harness.center(row(&harness, 2));
    harness.frame(vec![Input::PointerMoved(rest)]);
    harness.frame(Vec::new());
    assert!(
        focused(&harness, 2),
        "moving the pointer onto a row should focus it"
    );

    for _ in 0..15 {
        harness.key(Key::ArrowDown, Modifiers::NONE);
        harness.frame(Vec::new());
    }
    harness.frame(Vec::new());
    assert_ne!(
        under(&harness, rest),
        Some(2),
        "the menu should have scrolled rows under the resting pointer"
    );
    assert!(
        focused(&harness, 17),
        "a row that scrolled under the resting pointer took the focus from the keys"
    );

    let moved = rest + vec2(1.0, 0.0);
    harness.frame(vec![Input::PointerMoved(moved)]);
    harness.frame(Vec::new());
    let pointed = under(&harness, moved).expect("a row is under the pointer");
    assert!(
        focused(&harness, pointed),
        "moving the pointer again should focus the row under it"
    );
}
