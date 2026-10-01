use ra_ap_syntax::ast::{self, AstNode, HasArgList, HasAttrs, HasName};
use ra_ap_syntax::{SourceFile, SyntaxNode};

const BUILT_TYPES: [&str; 7] = [
    "NodeId",
    "ListChild",
    "CanvasItem",
    "GridCell",
    "TextItem",
    "Layer",
    "DynamicSegment",
];

const SIGNAL_WRITES: [&str; 3] = ["set", "update", "set_unconditionally"];

pub(crate) fn beui_violations(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let at = |node: &SyntaxNode| {
        let offset = u32::from(node.text_range().start()) as usize;
        let line = source[..offset].matches('\n').count() + 1;
        format!("{relative}:{line}")
    };
    let mut violations = Vec::new();
    for function in tree.syntax().descendants().filter_map(ast::Fn::cast) {
        let name = function
            .name()
            .map_or_else(String::new, |name| name.text().to_string());
        if is_component(&function) {
            if let Some(problem) = view_shape(&function) {
                violations.push(format!(
                    "component view: {} `{name}` {problem}",
                    at(function.syntax())
                ));
            }
        } else if builds_nodes(&function) {
            violations.push(format!(
                "component attribute: {} `{name}` builds part of a view but is not a #[component]",
                at(function.syntax())
            ));
        }
    }
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
    violations
}

fn is_component(function: &ast::Fn) -> bool {
    function.attrs().any(|attribute| {
        attribute
            .path()
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref())
            .is_some_and(|name| name.text() == "component")
    })
}

fn builds_nodes(function: &ast::Fn) -> bool {
    if function
        .syntax()
        .parent()
        .is_some_and(|parent| ast::AssocItemList::can_cast(parent.kind()))
    {
        return false;
    }
    let takes_document = function.param_list().is_some_and(|parameters| {
        parameters.params().any(|parameter| {
            parameter
                .ty()
                .is_some_and(|ty| ty.syntax().text().to_string().contains("Document"))
        })
    });
    let builds_a_view = function.body().is_some_and(|body| {
        body.syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .any(|call| is_view(&call))
    });
    if takes_document || !builds_a_view {
        return false;
    }
    let Some(ast::Type::PathType(returned)) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };
    returned
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| BUILT_TYPES.contains(&name.text()))
}

fn view_shape(function: &ast::Fn) -> Option<&'static str> {
    let body = function.body()?;
    let views = body
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| is_view(call) && belongs_to_body(call.syntax(), body.syntax()))
        .collect::<Vec<_>>();
    match views.as_slice() {
        [] => None,
        [view] => {
            let tail = body.stmt_list()?.tail_expr();
            let ends_with_it = tail.is_some_and(|tail| match tail {
                ast::Expr::MacroExpr(expr) => expr
                    .macro_call()
                    .is_some_and(|call| call.syntax() == view.syntax()),
                _ => false,
            });
            (!ends_with_it).then_some("must end with its one `view!` and have nothing after it")
        }
        _ => Some("builds more than one `view!`; choose between trees with `Show` or `Dynamic`"),
    }
}

fn is_view(call: &ast::MacroCall) -> bool {
    call.path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == "view")
}

fn belongs_to_body(node: &SyntaxNode, body: &SyntaxNode) -> bool {
    for ancestor in node.ancestors().skip(1) {
        if &ancestor == body {
            return true;
        }
        if ast::ClosureExpr::can_cast(ancestor.kind()) || ast::Fn::can_cast(ancestor.kind()) {
            return false;
        }
    }
    false
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
