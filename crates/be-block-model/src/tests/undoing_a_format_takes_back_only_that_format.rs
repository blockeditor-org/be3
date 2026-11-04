use super::*;

#[test]
fn undoing_a_format_takes_back_only_that_format() {
    let mut document = caption("hello world", Vec::new());
    let mine = format(&document, 1, 0..11, "bold", Some(MarkValue::Bool(true)));
    let step = document.step(&mine).expect("formatting changes something");
    document.apply(&mine);
    let theirs = format(&document, 2, 6..11, "italic", Some(MarkValue::Bool(true)));
    document.apply(&theirs);
    let italic = Seed {
        key: Name::new("italic"),
        ..bold(6..11)
    };

    document.apply(&step.undo());
    assert_eq!(seeds(&document), vec![italic.clone()]);

    document.apply(&step.redo());
    assert_eq!(seeds(&document), vec![bold(0..11), italic.clone()]);

    let unbold = format(&document, 2, 0..5, "bold", None);
    document.apply(&unbold);
    assert_eq!(seeds(&document), vec![bold(5..11), italic.clone()]);
    let step = document.step(&unbold);
    assert!(step.is_none(), "a format already applied changes nothing");
}
