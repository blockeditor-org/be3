use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use beui_host::native::build;

#[test]
fn its_actions_are_told_apart_by_place() {
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let chosen = chosen.clone();
        move || {
            view! {
                <Root>
                    <LockScreen
                        open=true
                        time="09:41"
                        date="Thursday, 8 October"
                        user="Ada Lovelace"
                        actions={vec![suspend(), suspend()]}
                        on_action={move |index: usize| chosen.borrow_mut().push(index)}
                    />
                </Root>
            }
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(800.0, 480.0));
    test.frame(Vec::new());
    test.click("lock.action.1");
    test.frame(Vec::new());
    test.click("lock.action.0");
    test.frame(Vec::new());
    assert_eq!(
        *chosen.borrow(),
        vec![1, 0],
        "two actions that look the same are still two actions"
    );
}
