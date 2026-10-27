use super::*;

#[test]
fn a_comment_is_skipped_before_its_quotes_are_read() {
    let script = "# the day's cell, and \"half a quote\n  # indented: it's a comment too\nclick 'GridCell \"Friday\"'\n\n";
    assert_eq!(
        commands(script).unwrap(),
        vec![(
            "click 'GridCell \"Friday\"'",
            vec!["click".to_owned(), "GridCell \"Friday\"".to_owned()]
        )]
    );
}
