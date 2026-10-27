use beui_core::runner::Hand;

use super::*;
use crate::keyboard::styled::close_center;
use crate::reactive::*;
use crate::styled::List;
use crate::styled::*;

#[test]
fn the_space_host_keeps_the_keyboard_open_until_its_close_button_is_pressed() {
    let _root = RootScope::new();
    let document = build_node(move || {
        let name = create_signal(String::new());
        view! {
            <List spacing=Space::None>
                <TextInput @test_id="field" value={Binding::new(name, move |next: String| name.set(next))} />
            </List>
        }
    });
    let mut space = InSpace::new(document);
    space.settle();
    let field = {
        let document = space.document.borrow();
        let field = document.find_test_id("field").expect("the field is shown");
        document.node_rect(field).expect("the field is laid out")
    };
    let keyboards = |space: &InSpace| space.space.borrow().keyboards().to_vec();

    space.aim(Hand::Right, crate::WindowId::MAIN, field.center());
    space.tap(Hand::Right);
    let opened = keyboards(&space);
    assert_eq!(opened.len(), 1, "a laser tapping the field opens the keyboard");

    space.aim(Hand::Right, crate::WindowId::MAIN, pos2(-2000.0, field.center().y));
    space.tap(Hand::Right);
    assert_eq!(
        space.space.borrow().focused(),
        None,
        "a tap on nothing takes the focus from the field's panel"
    );
    assert_eq!(
        keyboards(&space),
        opened,
        "in space the keyboard stays where it was put, though nothing asks for it"
    );

    let keys = Rect::from_min_size(Pos2::ZERO, crate::keyboard_host::WINDOW_SIZE);
    space.aim(Hand::Right, opened[0], close_center(keys));
    space.tap(Hand::Right);
    assert_eq!(keyboards(&space), [], "its close button closes it");

    space.aim(Hand::Right, crate::WindowId::MAIN, field.center());
    space.tap(Hand::Right);
    assert_eq!(
        keyboards(&space).len(),
        1,
        "and tapping the field brings it back"
    );
}
