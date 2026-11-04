use super::*;
use crate::{PasteKind, Pasted, PickedFile, native::PasteRequest};

#[test]
fn a_paste_brings_the_first_kind_the_focus_accepts_unless_the_platform_pastes_itself() {
    let mut intaken = Intaken::new();
    let image = PickedFile {
        name: "Pasted Image.png".to_owned(),
        data: vec![0x89, b'P', b'N', b'G'],
    };
    intaken.platform.clipboard = Some("pasted".to_owned());
    intaken.platform.clipboard_image = Some(image.clone());
    let paste = |intaken: &mut Intaken, request: PasteRequest, kinds: &[PasteKind]| {
        *intaken.ask.borrow_mut() = Some((request, kinds.to_vec()));
        intaken
            .frame(1.0, [])
            .into_iter()
            .filter(|event| matches!(event, Event::Paste(_)))
            .collect::<Vec<_>>()
    };

    let seen = paste(
        &mut intaken,
        PasteRequest::Shortcut,
        &[PasteKind::Image, PasteKind::Text],
    );
    assert_eq!(seen, [Event::Paste(Pasted::Image(image))]);

    let seen = paste(&mut intaken, PasteRequest::Asked, &[PasteKind::Text]);
    assert_eq!(seen, [Event::Paste(Pasted::Text("pasted".to_owned()))]);

    intaken.platform.clipboard_image = None;
    let seen = paste(&mut intaken, PasteRequest::Asked, &[PasteKind::Image]);
    assert!(
        seen.is_empty(),
        "a clipboard without an accepted kind pastes nothing: {seen:?}"
    );

    intaken.platform.delivers_paste = true;
    intaken.platform.pastes.clear();
    let seen = paste(&mut intaken, PasteRequest::Shortcut, &[PasteKind::Text]);
    assert!(
        seen.is_empty() && intaken.platform.pastes.is_empty(),
        "a platform that pastes on its own is not asked to paste the shortcut again: {seen:?}"
    );
    let seen = paste(&mut intaken, PasteRequest::Asked, &[PasteKind::Text]);
    assert_eq!(seen, [Event::Paste(Pasted::Text("pasted".to_owned()))]);

    intaken.platform.pastes.clear();
    paste(&mut intaken, PasteRequest::Asked, &[]);
    assert!(
        intaken.platform.pastes.is_empty(),
        "a focus that accepts nothing does not read the clipboard"
    );
}
