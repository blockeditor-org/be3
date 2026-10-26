use beui_core::runner::Hand;

use super::*;
use crate::reactive::*;
use crate::keyboard::styled::key_center;
use crate::keyboard::unstyled::{Layer, Value};
use crate::styled::*;
use crate::styled::List;

#[test]
fn the_space_host_opens_the_keyboard_as_a_panel_lasers_type_on_only_while_controllers_are_active() {
    let _root = RootScope::new();
    let typed = Rc::new(RefCell::new(String::new()));
    let seen = Rc::clone(&typed);
    let document = build_node(move || {
        let name = create_signal(String::new());
        view! {
            <List spacing=Space::None>
                <TextInput
                    @test_id="field"
                    value={Binding::new(name, move |next: String| {
                        *seen.borrow_mut() = next.clone();
                        name.set(next);
                    })}
                />
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

    let clicked = space.on_screen(crate::WindowId::MAIN, field.center());
    space.frame([Input::PointerMoved(clicked)]);
    for pressed in [true, false] {
        space.frame([Input::PointerButton {
            pos: clicked,
            button: PointerButton::Primary,
            pressed,
        }]);
    }
    space.settle();
    assert!(
        space.document.borrow().focused_node().is_some(),
        "the mouse focuses the field"
    );
    assert_eq!(
        space.space.borrow().keyboards(),
        [],
        "with only a mouse and keyboard the physical keyboard types and no panel opens"
    );

    space.aim(Hand::Right, crate::WindowId::MAIN, field.center());
    assert_eq!(
        space.space.borrow().keyboard_attached(),
        Some(false),
        "a controller's laser counts as no keyboard attached"
    );
    space.tap(Hand::Right);
    let keyboard = {
        let space = space.space.borrow();
        let keyboards = space.keyboards();
        assert_eq!(
            keyboards.len(),
            1,
            "a laser tapping the field opens the keyboard as a panel"
        );
        keyboards[0]
    };
    {
        let shell = space.space.borrow();
        assert_eq!(
            shell.focused(),
            Some(crate::WindowId::MAIN),
            "the keyboard's panel does not take the focus"
        );
        let main = shell.panel(crate::WindowId::MAIN).unwrap();
        let panel = shell.panel(keyboard).unwrap();
        let top = panel.world(pos2(panel.size.x / 2.0, 0.0));
        let under = main.world(pos2(field.center().x, field.bottom()));
        assert!(
            top.y < under.y && (top.x - under.x).abs() < 1e-3,
            "it opens just under the field: {top:?} under {under:?}"
        );
        assert!(
            main.normal().dot(top - under) > 0.0,
            "in front of the field's panel"
        );
    }
    let focused = space.document.borrow().focused_node();
    assert!(focused.is_some());

    let keys = Rect::from_min_size(Pos2::ZERO, crate::keyboard_host::WINDOW_SIZE);
    for letter in ['h', 'i'] {
        let key = key_center(keys, Layer::Text, Value::Char(letter)).expect("a letter key");
        space.aim(Hand::Right, keyboard, key);
        space.tap(Hand::Right);
    }
    space.settle();
    assert_eq!(*typed.borrow(), "hi", "the laser types on its keys");
    assert_eq!(
        space.space.borrow().focused(),
        Some(crate::WindowId::MAIN),
        "and the focus stays on the field's panel"
    );
    assert_eq!(space.document.borrow().focused_node(), focused);
}
