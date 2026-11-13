use super::*;
use std::cell::RefCell;
use std::rc::Rc;

use beui_types::ClipItem;
use beui_types::clipboard::formats;

#[test]
fn a_paste_of_the_text_this_page_copied_answers_the_rest_of_its_clip() {
    let blocks = formats::BLOCKS;
    copy(
        &beui_types::Clip::new()
            .with(ClipItem::new(blocks, b"rich".to_vec()))
            .with(ClipItem::text("plain")),
    );
    assert!(
        held(Some("other")).is_none(),
        "another program's text drops the clip"
    );
    assert!(
        held(Some("plain")).is_none(),
        "a dropped clip stays dropped"
    );

    copy(
        &beui_types::Clip::new()
            .with_lazy(blocks, |answer| answer(Some(b"rendered".to_vec().into())))
            .with(ClipItem::text("plain")),
    );
    let clip = held(Some("plain")).expect("the clipboard still holds what this page wrote");
    let pasted = Rc::new(RefCell::new(None));
    let into = Rc::clone(&pasted);
    clip.resolve(
        &[blocks.to_owned(), formats::TEXT.to_owned()],
        move |item| {
            *into.borrow_mut() = item;
        },
    );
    let pasted = pasted
        .borrow_mut()
        .take()
        .expect("the clip answers a be3 format");
    assert_eq!(pasted.format, blocks);
    assert_eq!(pasted.data, b"rendered");

    copy(&beui_types::Clip::new().with(ClipItem::new(blocks, b"rich".to_vec())));
    assert!(
        held(Some("")).is_some(),
        "a clip without text cleared the clipboard's text, and an empty paste is still its"
    );
    assert!(held(None).is_some(), "as is a paste with no text at all");
}
