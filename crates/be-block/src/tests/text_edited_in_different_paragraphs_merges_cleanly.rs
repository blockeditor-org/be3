use super::*;

#[test]
fn text_edited_in_different_paragraphs_merges_cleanly() {
    let base =
        TextContent::from("# Title\n\nfirst paragraph\n\nsecond paragraph\n\nthird paragraph\n");
    let ours = TextContent::from(
        "# Title\n\nfirst paragraph, edited\n\nsecond paragraph\n\nthird paragraph\n",
    );
    let theirs = edited(
        &TextContent::from(
            "# Title\n\nfirst paragraph\n\nsecond paragraph\n\nthird paragraph, edited\n",
        ),
        [TextOp::SetLanguage(TextLanguage::PlainText)],
    );

    let (result, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        result.text(),
        "# Title\n\nfirst paragraph, edited\n\nsecond paragraph\n\nthird paragraph, edited\n"
    );
    assert_eq!(result.language(), TextLanguage::PlainText);
    let (same, conflicts) = merged(&base, &ours, &ours);
    assert_eq!((same, conflicts), (ours, 0));
}
