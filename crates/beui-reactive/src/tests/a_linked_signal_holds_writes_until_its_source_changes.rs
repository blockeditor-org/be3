use super::*;
use crate::create_linked_signal;

#[test]
fn a_linked_signal_holds_writes_until_its_source_changes() {
    let _root = RootScope::new();
    let saved = create_signal("first".to_owned());
    let draft = create_linked_signal(move || saved.get());
    assert_eq!(draft.get_untracked(), "first", "it starts from its source");
    draft.set("edited".to_owned());
    flush();
    assert_eq!(draft.get_untracked(), "edited", "a write holds while the source is still");
    saved.set("second".to_owned());
    flush();
    assert_eq!(draft.get_untracked(), "second", "a change to the source resets it");
}

#[test]
fn a_linked_signal_can_keep_part_of_what_was_written_when_it_resets() {
    let _root = RootScope::new();
    let value = create_signal(1.0_f32);
    let text = create_linked_signal(move |typed: Option<&String>| {
        let value = value.get();
        match typed {
            Some(typed) if typed.parse::<f32>().ok() == Some(value) => typed.clone(),
            _ => value.to_string(),
        }
    });
    assert_eq!(text.get_untracked(), "1");
    text.set("2.0".to_owned());
    value.set(2.0);
    flush();
    assert_eq!(text.get_untracked(), "2.0", "text that still means the value is kept");
    value.set(3.0);
    flush();
    assert_eq!(text.get_untracked(), "3");
}
