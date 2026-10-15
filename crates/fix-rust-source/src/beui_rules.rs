use ra_ap_syntax::ast::{self, AstNode, HasArgList};
use ra_ap_syntax::{SourceFile, SyntaxNode};

const SIGNAL_WRITES: [&str; 3] = ["set", "update", "set_unconditionally"];

const PRIMITIVE_USERS: [&str; 7] = [
    "crates/beui/",
    "crates/beui-view/",
    "crates/beui-components-styled/",
    "crates/beui-components-unstyled/",
    "crates/beui-inspector/",
    "crates/beui-renderer-wgpu/",
    "crates/beui-demo/src/base_pages.rs",
];

pub(crate) fn beui_violations(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let at = |node: &SyntaxNode| {
        let offset = u32::from(node.text_range().start()) as usize;
        let line = source[..offset].matches('\n').count() + 1;
        format!("{relative}:{line}")
    };
    let mut violations = Vec::new();
    let test_source = relative.contains("/tests/") || relative.ends_with("/tests.rs");
    for call in tree.syntax().descendants().filter_map(ast::CallExpr::cast) {
        if test_source || !calls(&call, "create_memo") {
            continue;
        }
        let Some(arguments) = call.arg_list() else {
            continue;
        };
        for write in arguments
            .syntax()
            .descendants()
            .filter_map(ast::MethodCallExpr::cast)
        {
            if write
                .name_ref()
                .is_some_and(|name| SIGNAL_WRITES.contains(&name.text()))
            {
                violations.push(format!(
                    "memo write: {} a memo must not write signals",
                    at(write.syntax())
                ));
            }
        }
    }
    if !PRIMITIVE_USERS
        .iter()
        .any(|allowed| relative.starts_with(allowed))
    {
        violations.extend(primitive_uses(tree, source, relative));
    }
    violations
}

fn primitive_uses(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let tokens: Vec<_> = tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && !matches!(token.text(), ":" | "::"))
        .collect();
    tokens
        .windows(2)
        .filter(|pair| {
            matches!(pair[0].text(), "beui" | "beui_view") && pair[1].text() == "primitives"
        })
        .map(|pair| {
            let offset = u32::from(pair[1].text_range().start()) as usize;
            let line = source[..offset].matches('\n').count() + 1;
            format!(
                "primitive outside beui: {relative}:{line} only beui's own crates use beui::primitives; import beui::styled::* instead"
            )
        })
        .collect()
}

fn calls(call: &ast::CallExpr, function: &str) -> bool {
    let Some(ast::Expr::PathExpr(callee)) = call.expr() else {
        return false;
    };
    callee
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == function)
}
