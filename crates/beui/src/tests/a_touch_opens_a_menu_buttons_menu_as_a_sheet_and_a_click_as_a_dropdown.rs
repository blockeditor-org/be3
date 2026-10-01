use super::*;
use crate::reactive::view;
use crate::styled::MenuButton;
use crate::unstyled::MenuItem;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

fn row(harness: &Harness, label: &str) -> Pos2 {
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, label)
        .unwrap_or_else(|| panic!("the menu lists {label}"));
    harness.center(row)
}

#[test]
fn a_touch_opens_a_menu_buttons_menu_as_a_sheet_and_a_click_as_a_dropdown() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let chosen = picked.clone();
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <MenuButton
                label="Sort"
                items={view! {
                    <MenuItem label="By name" />
                    <MenuItem label="By date" />
                }}
                on_select={move |path: Vec<usize>| chosen.borrow_mut().push(path)}
            />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.settle();

    harness.click(harness.center(button));
    harness.settle();
    assert!(
        !laid_out(&harness, "sheet.handle"),
        "a mouse click opens the menu under the button, not a sheet"
    );
    harness.click(row(&harness, "By date"));
    harness.settle();
    assert_eq!(*picked.borrow(), [vec![1]]);

    let at = harness.center(button);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    harness.settle();
    assert!(
        laid_out(&harness, "sheet.handle"),
        "a tap opens the menu as a sheet"
    );
    let first = row(&harness, "By name");
    harness.touch(TouchPhase::Start, first);
    harness.touch(TouchPhase::End, first);
    harness.settle();
    assert_eq!(*picked.borrow(), [vec![1], vec![0]]);
    assert!(
        !laid_out(&harness, "sheet.handle"),
        "choosing an item closes the sheet"
    );
}
