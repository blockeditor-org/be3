use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_color_input_reports_the_hex_it_was_typed() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [_input]) = toolbar_of(|| {
        [view! {
            <ColorInput
                value={Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0xFF)}
                on_change={move |color| set_changes.update(|changes| changes.push(color))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(changes.with_untracked(|changes| changes.is_empty()));

    harness.type_text("0");
    harness.frame(Vec::new());

    assert_eq!(
        changes.get_untracked(),
        [Color32::from_rgba_unmultiplied(0x11, 0x22, 0x30, 0xFF)]
    );
}
