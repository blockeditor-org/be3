use super::*;

#[test]
fn tokenize_deref_is_an_access() {
    let (result, _source) = tokenize_str("x.*");
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let [SyntaxNode::Identifier(x), SyntaxNode::Identifier(deref)] = result.result.as_slice()
    else {
        panic!("expected two identifiers, got {:?}", result.result);
    };
    assert_eq!(x.str, "x");
    assert_eq!(deref.ident_tag, IdentifierTag::Access);
    assert_eq!(deref.str, "*");
}
