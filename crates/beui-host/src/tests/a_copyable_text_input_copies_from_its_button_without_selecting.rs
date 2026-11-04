use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_copyable_text_input_copies_from_its_button_without_selecting() {
    let _root = crate::reactive::RootScope::new();
    let text = "a secret much longer than the field is wide, so it scrolls sideways".to_owned();
    let value = create_signal(text.clone());
    let api = ComponentRef::new();
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <TextInput api={api} value={value} copyable=true />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(input);
    let at = Pos2::new(rect.right() - 14.0, rect.center().y);
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.frame(vec![Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    let clicked = harness.frame(vec![Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
    }]);

    assert_eq!(clicked.copied_text.as_deref(), Some(text.as_str()));
    let api: crate::styled::TextInputApi = api.get();
    assert!(api.selection().is_empty(), "the press stops at the button");
    assert!(!api.focused().get_untracked());
}
