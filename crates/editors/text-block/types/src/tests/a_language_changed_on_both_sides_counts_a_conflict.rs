use super::*;

#[test]
fn a_language_changed_on_both_sides_counts_a_conflict() {
    let base = TextBlock::of("fn main() {}\n");
    let ours = edited(&base, [TextBlock::set_language(TextLanguage::Rust)]);
    let theirs = edited(&base, [TextBlock::set_language(TextLanguage::Zig)]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(merged.root().language, TextLanguage::Rust);
    assert_eq!(TextBlock::text(&merged), "fn main() {}\n");
}
