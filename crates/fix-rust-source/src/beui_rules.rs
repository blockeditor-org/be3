use ra_ap_syntax::ast::{self, AstNode, HasArgList, HasAttrs};
use ra_ap_syntax::{SourceFile, SyntaxNode};

const SIGNAL_WRITES: [&str; 3] = ["set", "update", "set_unconditionally"];

const PRIMITIVE_USERS: [&str; 14] = [
    "crates/beui/",
    "crates/beui-view/",
    "crates/beui-components-styled/",
    "crates/beui-components-unstyled/",
    "crates/beui-inspector/",
    "crates/beui-renderer-wgpu/",
    "crates/beui-demo/src/base_pages.rs",
    "crates\\beui\\",
    "crates\\beui-view\\",
    "crates\\beui-components-styled\\",
    "crates\\beui-components-unstyled\\",
    "crates\\beui-inspector\\",
    "crates\\beui-renderer-wgpu\\",
    "crates\\beui-demo\\src\\base_pages.rs",
];

pub(crate) fn beui_violations(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let at = |node: &SyntaxNode| {
        let offset = u32::from(node.text_range().start()) as usize;
        let line = source[..offset].matches('\n').count() + 1;
        format!("{relative}:{line}")
    };
    let mut violations = Vec::new();
    let test_source = relative.contains("/tests/")
        || relative.ends_with("/tests.rs")
        || relative.contains("\\tests\\")
        || relative.ends_with("\\tests.rs");
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
    violations.extend(not_marked_component(tree, source, relative));
    violations
}

fn not_marked_component(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    tree.syntax().descendants().filter_map(ast::Fn::cast).filter(|function| !function
            .syntax()
            .ancestors()
            .any(|node| ast::Impl::can_cast(node.kind()) || ast::Trait::can_cast(node.kind())))
        .filter(|function| !function.attrs().any(|attr| {
            attr.as_simple_path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref())
                .is_some_and(|name_ref| name_ref.text() == "component")})).filter(|function| function
            .body()
            .and_then(|body| body.tail_expr())
            .and_then(|expr| match expr {
                ast::Expr::MacroExpr(mac) => Some(mac),
                _ => None,
            })
            .and_then(|mac| mac.macro_call())
            .and_then(|call| call.path())
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref())
            .is_some_and(|name_ref| name_ref.text() == "view")).map(|function|  {
        let line_str = function
            .fn_token()
            .map(|fn_token| {
                let offset = u32::from(fn_token.text_range().start()) as usize;
                let line_num = source[..offset].matches('\n').count() + 1;
                line_num.to_string()
            })
            .unwrap_or_else(|| "?".to_string());

        format!(
            "component function not marked #[component]: {relative}:{line_str} a function that returns a beui node should be a component.\nsolution: rename it to PascalCase and mark it #[component], then call it from inside `view! {{ <FunctionName /> }}`"
        )
    }).collect()
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
