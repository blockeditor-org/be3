use super::*;

fn single_binary(items: &[SyntaxNode], tag: OpTag) -> &BinaryExpressionToken {
    let items: Vec<&SyntaxNode> = items
        .iter()
        .filter(|n| !matches!(n, SyntaxNode::Whitespace(_)))
        .collect();
    let [SyntaxNode::BinaryExpression(binary)] = items.as_slice() else {
        panic!("expected a single binary expression, got {items:?}");
    };
    assert_eq!(binary.tag, tag);
    binary
}

fn segment(binary: &BinaryExpressionToken, index: usize) -> &[SyntaxNode] {
    let SyntaxNode::OperatorSegment(seg) = &binary.items[index] else {
        panic!(
            "expected an operator segment, got {:?}",
            binary.items[index]
        );
    };
    &seg.items
}

#[test]
fn tokenize_arithmetic_operators_nest_by_precedence() {
    let (result, _source) = tokenize_str("a == b + c * d - e");
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let compare = single_binary(&result.result, OpTag::Compare);
    assert_eq!(compare.items.len(), 3);
    let add = single_binary(segment(compare, 2), OpTag::Add);
    assert_eq!(add.items.len(), 5);
    assert!(matches!(&add.items[1], SyntaxNode::Operator(o) if o.op == "+"));
    assert!(matches!(&add.items[3], SyntaxNode::Operator(o) if o.op == "-"));
    let mul = single_binary(segment(add, 2), OpTag::Mul);
    assert_eq!(mul.items.len(), 3);
}
