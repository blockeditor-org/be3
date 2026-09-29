use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::MenuButton;

#[test]
fn a_press_and_release_in_one_frame_on_a_submenu_item_selects_it_once() {
    let button = NodeRef::new();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let (document, _) = toolbar_of({
        let button = button.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label="Profiles">
                    <unstyled::MenuItem label="Profile 1" />
                    <unstyled::MenuItem label="Profile 2" />
                </unstyled::MenuItem>
            };
            [view! {
                <MenuButton
                    @node_ref=&button
                    label="More"
                    items
                    on_select={move |path: Vec<usize>| {
                        sink.borrow_mut().push(path);
                    }}
                />
            }]
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(button));
    harness.frame(Vec::new());
    let find = |harness: &Harness, label: &str| {
        let document = harness.document();
        document
            .overlay_stack
            .iter()
            .find_map(|overlay| text_within(document, *overlay, label))
            .unwrap_or_else(|| panic!("the menu shows {label}"))
    };
    let profiles = find(&harness, "Profiles");
    harness.frame(vec![Event::PointerMoved(harness.center(profiles))]);
    harness.frame(Vec::new());
    let second = harness.center(find(&harness, "Profile 2"));
    harness.frame(vec![Event::PointerMoved(second)]);
    harness.frame(vec![
        Event::PointerButton {
            pos: second,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: second,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    harness.frame(Vec::new());

    assert_eq!(selected.borrow().as_slice(), &[vec![0usize, 1usize]]);
}
