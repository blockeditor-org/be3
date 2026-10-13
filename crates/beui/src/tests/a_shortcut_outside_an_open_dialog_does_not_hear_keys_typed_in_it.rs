use super::*;
use crate::input::KeyPress;
use crate::reactive::{NodeRef, build, create_signal, on_shortcut, view, with_reactive_scope};
use crate::styled::{Button, ButtonVariant, Dialog};

#[test]
fn a_shortcut_outside_an_open_dialog_does_not_hear_keys_typed_in_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = Rc::new(Cell::new(0));
    let (open, set_open) = create_signal(false);
    let inside = NodeRef::new();
    let document = build({
        let (heard, inside) = (heard.clone(), inside.clone());
        move || {
            on_shortcut(move |press: KeyPress| {
                if press.pressed && press.key == Key::K && press.modifiers.ctrl {
                    heard.set(heard.get() + 1);
                }
                false
            });
            view! {
                <List spacing=0.0>
                    <Dialog open={open} on_dismiss={|| {}}>
                        <Button @node_ref=&inside variant=ButtonVariant::Primary label="Inside" />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::K, Modifiers::CTRL);
    assert_eq!(
        heard.get(),
        1,
        "with nothing open the shortcut hears the key"
    );

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    harness.click(harness.center(inside.get()));
    harness.key(Key::K, Modifiers::CTRL);
    assert_eq!(
        heard.get(),
        1,
        "the dialog is a boundary on the focus path, so the shortcut outside it hears nothing"
    );
}
