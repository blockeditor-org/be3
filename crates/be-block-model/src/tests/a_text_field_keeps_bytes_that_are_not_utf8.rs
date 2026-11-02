use super::*;

#[test]
fn a_text_field_keeps_bytes_that_are_not_utf8() {
    let bytes = vec![0xff, 0x00, 0xc3, 0x28, b'\n', 0x80];
    let document = Document::new(&Note {
        body: Text::new(bytes.clone()),
        lines: List::default(),
    });

    let reloaded = Document::<Note>::from_bytes(&document.to_bytes()).expect("the bytes decode");

    assert_eq!(reloaded.root().body.bytes(), bytes);
}
