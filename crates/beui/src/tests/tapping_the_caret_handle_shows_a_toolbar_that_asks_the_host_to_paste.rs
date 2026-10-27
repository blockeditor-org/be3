use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tapping_the_caret_handle_shows_a_toolbar_that_asks_the_host_to_paste() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );
    let start = pos2(text.left() + 1.0, text.center().y);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .handles(harness.document());
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::End, handle);
    harness.frame(Vec::new());

    let toolbar = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .toolbar(harness.document())
        .expect("tapping the caret handle shows its toolbar");
    assert!(
        text_within(harness.document(), toolbar, "Copy").is_none(),
        "there is nothing to copy without a selection"
    );
    let paste = text_within(harness.document(), toolbar, "Paste").expect("the toolbar pastes");
    let row = harness.center(paste);
    harness.touch(TouchPhase::Start, row);
    let output = harness.touch(TouchPhase::End, row);
    assert_eq!(output.paste, Some(crate::native::PasteRequest::Asked));
    assert_eq!(
        output.main().window.requests.pastes,
        [crate::PasteKind::Text]
    );

    harness.frame(vec![Input::Paste(crate::Pasted::Text("Say ".to_owned()))]);
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "Say Hello"
    );
}
