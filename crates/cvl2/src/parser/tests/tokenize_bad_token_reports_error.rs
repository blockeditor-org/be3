use super::*;

#[test]
fn tokenize_bad_token_reports_error() {
    let (result, _source) = tokenize_str("#");

    let [SyntaxNode::Err(err)] = result.result.as_slice() else {
        panic!(
            "expected the bad token to stay in the tree: {:?}",
            result.result
        );
    };
    assert_eq!(err.text, "#");
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].entries[0].message, "bad token \"#\"");
    assert_eq!(result.errors[0].entries[0].style, ErrorStyle::Error);
}
