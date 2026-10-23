use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_color_input_takes_a_hex_typed_without_a_hash_and_keeps_its_opacity() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [_input]) = toolbar_of(|| {
        [view! {
            <ColorInput
                value={Binding::new(Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0x80), move |next| changes.update(|changes| changes.push(next)))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::A, Modifiers::CTRL);
    harness.type_text("ff8800");
    harness.frame(Vec::new());

    assert_eq!(
        changes.get_untracked().last(),
        Some(&Color32::from_rgba_unmultiplied(0xFF, 0x88, 0x00, 0x80))
    );
}
