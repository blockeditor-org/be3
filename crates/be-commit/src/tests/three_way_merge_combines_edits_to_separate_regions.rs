use super::*;

#[test]
fn three_way_merge_combines_edits_to_separate_regions() {
    let base = "alpha\nbravo\ncharlie\ndelta\necho\n";
    let ours = "ALPHA\nbravo\ncharlie\ndelta\necho\n";
    let theirs = "alpha\nbravo\ncharlie\ndelta\nECHO\n";

    let outcome = merge_lines(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
    assert!(outcome.is_clean(), "{:?}", outcome.conflicts);
    assert_eq!(text(&outcome), "ALPHA\nbravo\ncharlie\ndelta\nECHO\n");

    let inserted = merge_lines(
        base.as_bytes(),
        "alpha\nbravo\nnew\ncharlie\ndelta\necho\n".as_bytes(),
        "alpha\nbravo\ncharlie\ndelta\necho\ntail\n".as_bytes(),
    );
    assert!(inserted.is_clean(), "{:?}", inserted.conflicts);
    assert_eq!(
        text(&inserted),
        "alpha\nbravo\nnew\ncharlie\ndelta\necho\ntail\n"
    );

    let deleted = merge_lines(
        base.as_bytes(),
        "alpha\ncharlie\ndelta\necho\n".as_bytes(),
        "alpha\nbravo\ncharlie\ndelta\n".as_bytes(),
    );
    assert!(deleted.is_clean(), "{:?}", deleted.conflicts);
    assert_eq!(text(&deleted), "alpha\ncharlie\ndelta\n");
}
