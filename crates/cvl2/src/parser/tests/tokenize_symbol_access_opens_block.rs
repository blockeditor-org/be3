use super::*;

#[test]
fn tokenize_symbol_access_opens_block() {
    let (result, _source) = tokenize_str("a.[b]");
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let [SyntaxNode::Identifier(a), SyntaxNode::Block(access)] = result.result.as_slice() else {
        panic!(
            "expected an identifier and a block, got {:?}",
            result.result
        );
    };
    assert_eq!(a.str, "a");
    assert_eq!(access.tag, BracketTag::SymbolAccess);
    assert!(matches!(access.items.as_slice(), [SyntaxNode::Identifier(b)] if b.str == "b"));
}
