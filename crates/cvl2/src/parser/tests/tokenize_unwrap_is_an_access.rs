use super::*;

#[test]
fn tokenize_unwrap_is_an_access() {
    let (result, _source) = tokenize_str("x.?.?");
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let names: Vec<(&str, IdentifierTag)> = result
        .result
        .iter()
        .map(|node| match node {
            SyntaxNode::Identifier(id) => (id.str.as_str(), id.ident_tag),
            other => panic!("expected identifiers, got {other:?}"),
        })
        .collect();
    assert_eq!(
        names,
        [
            ("x", IdentifierTag::Normal),
            ("?", IdentifierTag::Access),
            ("?", IdentifierTag::Access),
        ]
    );
}
