use super::*;

#[test]
fn text_edited_in_different_paragraphs_merges_cleanly() {
    let base = TextBlock::of("# Title\n\nfirst paragraph\n\nsecond paragraph\n\nthird paragraph\n");
    let ours = TextBlock::of(
        "# Title\n\nfirst paragraph, edited\n\nsecond paragraph\n\nthird paragraph\n",
    );
    let theirs = edited(
        &TextBlock::of(
            "# Title\n\nfirst paragraph\n\nsecond paragraph\n\nthird paragraph, edited\n",
        ),
        [TextBlock::set_language(TextLanguage::PlainText)],
    );

    let (result, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        TextBlock::text(&result),
        "# Title\n\nfirst paragraph, edited\n\nsecond paragraph\n\nthird paragraph, edited\n"
    );
    assert_eq!(result.root().language, TextLanguage::PlainText);
    let (same, conflicts) = merged(&base, &ours, &ours);
    assert_eq!((same, conflicts), (ours, 0));
}
