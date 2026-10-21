use super::*;

#[test]
fn formatting_and_typing_at_once_converge_in_either_order() {
    let base = caption("hello world", Vec::new());
    let emboldened = format(&base, 1, 6..11, "bold", Some(MarkValue::Bool(true)));
    let typed = type_into(&base, 2, 8, "--");
    let italic = format(&base, 2, 0..5, "italic", Some(MarkValue::Bool(true)));

    let mut first = base.clone();
    first.apply(&emboldened);
    first.apply(&typed);
    first.apply(&italic);
    let mut second = base.clone();
    second.apply(&typed);
    second.apply(&italic);
    second.apply(&emboldened);

    assert_eq!(words(&first), "hello wo--rld");
    assert_eq!(first, second);
    let italic_seed = Seed {
        key: Name::new("italic"),
        ..bold(0..5)
    };
    assert_eq!(seeds(&first), vec![italic_seed, bold(6..13)]);
}
