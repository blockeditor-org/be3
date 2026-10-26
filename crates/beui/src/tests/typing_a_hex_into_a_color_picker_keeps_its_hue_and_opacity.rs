use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn typing_a_hex_into_a_color_picker_keeps_its_hue_and_opacity() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorPicker
                    value={Binding::new(Color32::from_rgba_unmultiplied(0x20, 0x40, 0x80, 0x80), move |next| changes.update(|changes| changes.push(next)))}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    harness.type_text("#ff8800");

    assert_eq!(
        changes.get_untracked().last(),
        Some(&Color32::from_rgba_unmultiplied(0xFF, 0x88, 0x00, 0x80))
    );
}
