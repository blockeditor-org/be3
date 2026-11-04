use super::*;
use crate::clipboard::formats;
use crate::{Clip, ClipItem, native::PasteRequest};

fn owned(formats: &[&str]) -> Vec<String> {
    formats.iter().map(|format| (*format).to_owned()).collect()
}

#[test]
fn a_paste_brings_the_first_kind_the_focus_accepts_unless_the_platform_pastes_itself() {
    let mut intaken = Intaken::new();
    let image = ClipItem::png(vec![0x89, b'P', b'N', b'G']);
    intaken.platform.clipboard = Some(Clip::text("pasted").with(image.clone()));
    let paste = |intaken: &mut Intaken, request: PasteRequest, formats: &[&str]| {
        *intaken.ask.borrow_mut() = Some((request, owned(formats)));
        intaken
            .frame(1.0, [])
            .into_iter()
            .filter(|event| matches!(event, Event::Paste(_)))
            .collect::<Vec<_>>()
    };
    let shortcut = PasteRequest::Shortcut { plain: false };

    let seen = paste(&mut intaken, shortcut, &[formats::PNG, formats::TEXT]);
    assert_eq!(seen, [Event::Paste(image)]);

    let seen = paste(
        &mut intaken,
        PasteRequest::Shortcut { plain: true },
        &[formats::PNG, formats::TEXT],
    );
    assert_eq!(
        seen,
        [Event::Paste(ClipItem::text("pasted"))],
        "Ctrl+Shift+V pastes plain text even where the focus prefers more"
    );
    let seen = paste(
        &mut intaken,
        PasteRequest::Shortcut { plain: true },
        &[formats::PNG],
    );
    assert!(
        seen.is_empty(),
        "nor anything to a focus without text: {seen:?}"
    );

    let seen = paste(&mut intaken, PasteRequest::Asked, &[formats::TEXT]);
    assert_eq!(seen, [Event::Paste(ClipItem::text("pasted"))]);

    intaken.platform.clipboard = Some(Clip::text("pasted"));
    let seen = paste(&mut intaken, PasteRequest::Asked, &[formats::PNG]);
    assert!(
        seen.is_empty(),
        "a clipboard without an accepted format pastes nothing: {seen:?}"
    );

    intaken.platform.delivers_paste = true;
    intaken.platform.pastes.clear();
    let seen = paste(&mut intaken, shortcut, &[formats::TEXT]);
    assert!(
        seen.is_empty() && intaken.platform.pastes.is_empty(),
        "a platform that pastes on its own is not asked to paste the shortcut again: {seen:?}"
    );
    let seen = paste(&mut intaken, PasteRequest::Asked, &[formats::TEXT]);
    assert_eq!(seen, [Event::Paste(ClipItem::text("pasted"))]);

    intaken.platform.pastes.clear();
    paste(&mut intaken, PasteRequest::Asked, &[]);
    assert!(
        intaken.platform.pastes.is_empty(),
        "a focus that accepts nothing does not read the clipboard"
    );
}
