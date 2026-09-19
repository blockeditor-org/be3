use super::*;
use crate::reactive::view;
use crate::styled::ColorInput;

#[test]
fn a_color_input_reports_the_hex_it_was_typed() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [_input]) = toolbar_of(|| {
        [view! {
            <ColorInput
                value={Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0xFF)}
                on_change={move |color| sink.borrow_mut().push(color)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(changes.borrow().is_empty());

    harness.type_text("0");
    harness.frame(Vec::new());

    assert_eq!(
        *changes.borrow(),
        [Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0xF0)]
    );
}
