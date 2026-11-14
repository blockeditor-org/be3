use super::*;

#[test]
fn a_clipboard_with_a_size_limit_carries_what_fits_in_order() {
    let items = [("blocks", 40), ("canvas", 100), ("files", 50), ("text", 10)];
    let kept = fitting(items, 100, |(_, cost)| *cost)
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert_eq!(
        kept,
        ["blocks", "files", "text"],
        "what would overrun the budget is left out, and smaller ones after it still go"
    );
    assert!(fitting(items, 0, |(_, cost)| *cost).is_empty());
}
