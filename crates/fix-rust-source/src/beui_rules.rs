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
    if !test_source {
        violations.extend(component_uses(tree, source, relative));
    }
    violations
}

fn attribute_named(node: &impl HasAttrs, name: &str) -> bool {
    node.attrs().any(|attribute| {
        attribute
            .path()
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref())
            .is_some_and(|found| found.text() == name)
    })
}

fn component_uses(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for function in tree.syntax().descendants().filter_map(ast::Fn::cast) {
        if attribute_named(&function, "component") || attribute_named(&function, "test") {
            continue;
        }
        if function.syntax().ancestors().skip(1).any(|node| {
            ast::Trait::can_cast(node.kind())
                || ast::Impl::cast(node)
                    .is_some_and(|implementation| implementation.trait_().is_some())
        }) {
            continue;
        }
        let uncovered = function
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .any(|call| {
                if !call
                    .path()
                    .and_then(|path| path.segment())
                    .and_then(|segment| segment.name_ref())
                    .is_some_and(|name| name.text() == "view")
                {
                    return false;
                }
                let mut deferred = false;
                for parent in call.syntax().ancestors().skip(1) {
                    deferred |= ast::ClosureExpr::can_cast(parent.kind());
                    if let Some(owner) = ast::Fn::cast(parent.clone()) {
                        return owner.syntax() == function.syntax();
                    }
                    if let Some(renderer) = ast::MethodCallExpr::cast(parent.clone())
                        && renderer.name_ref().is_some_and(|name| {
                            matches!(name.text(), "with_sheet" | "with_toolbar")
                        })
                    {
                        return false;
                    }
                    if let Some(renderer) = ast::CallExpr::cast(parent)
                        && (calls(&renderer, "render")
                            || calls(&renderer, "render_fragment")
                            || calls(&renderer, "build")
                            || calls(&renderer, "enter")
                            || deferred_renderer(&renderer)
                            || (deferred && calls(&renderer, "store")))
                    {
                        return false;
                    }
                }
                false
            });
        if uncovered {
            let offset = u32::from(function.syntax().text_range().start()) as usize;
            let line = source[..offset].matches('\n').count() + 1;
            violations.push(format!("view outside component: {relative}:{line} a function using view! must be #[component], except a top-level renderer"));
        }
    }
    violations
}

fn deferred_renderer(call: &ast::CallExpr) -> bool {
    let Some(ast::Expr::PathExpr(expression)) = call.expr() else {
        return false;
    };
    let Some(path) = expression.path() else {
        return false;
    };
    let constructor = path.segment().and_then(|segment| segment.name_ref());
    let kind = path
        .qualifier()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());
    constructor.is_some_and(|name| name.text() == "new")
        && kind.is_some_and(|name| {
            matches!(
                name.text(),
                "Render" | "RenderFn" | "MenuStyle" | "ScrollbarStyle" | "CompletionMenu"
            )
        })
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
