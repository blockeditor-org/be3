use super::*;
use crate::{PasteKind, native::PasteRequest};

#[test]
fn a_typed_paste_types_the_clipboard_key_by_key() {
    let mut intaken = Intaken::new();
    intaken.platform.clipboard = Some("Hé".to_owned());
    *intaken.ask.borrow_mut() = Some((PasteRequest::Typed, Vec::new()));
    let seen = intaken.frame(1.0, []);
    let typed: Vec<&Event> = seen
        .iter()
        .filter(|event| matches!(event, Event::Key { text: Some(_), .. } | Event::Text(_)))
        .collect();
    assert!(
        matches!(typed.as_slice(), [Event::Key { text: Some(h), press, .. }, Event::Text(e)]
            if h == "H" && press.key == Key::H && e == "é"),
        "the clipboard arrives as the keys that type it, and as text where no key does: {typed:?}"
    );
    assert!(
        !seen.iter().any(|event| matches!(event, Event::Paste(_))),
        "nothing is pasted"
    );
    assert_eq!(intaken.platform.pastes, [vec![PasteKind::Text]]);
}
