use super::*;

#[test]
fn rich_text_marks_converge_in_any_order() {
    let text = Sequence::from_items(b"shared text".to_vec());
    let bytes = text.items();
    let bold = Marks::op(&text, &bytes, 0..6, Attribute::Bold, 1).unwrap();
    let plain = Marks::op(&text, &bytes, 3..11, Attribute::Bold, 0).unwrap();

    let mut left = Marks::default();
    let mut right = Marks::default();
    let first = left.mint(1, bold);
    let second = right.mint(2, plain);
    left.apply(second);
    right.apply(first);
    right.apply(first);

    assert_eq!(left, right);
    let runs = left.formats(&text, &bytes);
    let bolded: Vec<bool> = (0..bytes.len())
        .map(|at| format_at(&runs, at).unwrap().bold)
        .collect();
    let ties = first.id.clock == second.id.clock;
    assert!(ties, "both sides minted their first mark");
    assert_eq!(
        bolded,
        [
            true, true, true, false, false, false, false, false, false, false, false
        ],
        "the mark from the higher client wins where two marks of one clock overlap"
    );
}
