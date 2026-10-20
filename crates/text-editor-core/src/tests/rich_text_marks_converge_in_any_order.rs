use super::*;

#[test]
fn rich_text_marks_converge_in_any_order() {
    let text = Sequence::from_items(b"shared text".to_vec());
    let bytes = text.items();
    let bold = Name::new("bold");
    let op = |range: std::ops::Range<usize>, value: Option<Value>| {
        Marks::op(
            &text,
            &bytes,
            range,
            Scope::Inline,
            Expand::After,
            &bold,
            value,
        )
        .unwrap()
    };

    let mut left = Marks::default();
    let mut right = Marks::default();
    let first = left.mint(1, op(0..6, Some(Value::Bool(true))));
    let second = right.mint(2, op(3..11, None));
    left.apply(second.clone());
    right.apply(first.clone());
    right.apply(first.clone());

    assert_eq!(left, right);
    assert_eq!(
        first.id.clock, second.id.clock,
        "both sides minted their first mark"
    );
    let formats = left.formats(&text, &bytes);
    let bolded: Vec<bool> = (0..bytes.len())
        .map(|at| formats.marks_at(at).unwrap().get("bold").is_some())
        .collect();
    assert_eq!(
        bolded,
        [
            true, true, true, false, false, false, false, false, false, false, false
        ],
        "the mark from the higher client wins where two marks of one clock overlap"
    );
}
