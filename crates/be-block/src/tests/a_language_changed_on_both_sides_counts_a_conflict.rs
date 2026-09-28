use super::*;

#[test]
fn a_language_changed_on_both_sides_counts_a_conflict() {
    let base = TextContent::from("fn main() {}\n");
    let ours = edited(&base, [TextOp::SetLanguage(TextLanguage::Rust)]);
    let theirs = edited(&base, [TextOp::SetLanguage(TextLanguage::Zig)]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(merged.language(), TextLanguage::Rust);
    assert_eq!(merged.text(), "fn main() {}\n");
}
