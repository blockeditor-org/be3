use ra_ap_syntax::ast::{self, AstNode, HasArgList, HasAttrs};
use ra_ap_syntax::{SourceFile, SyntaxNode};

const SIGNAL_WRITES: [&str; 3] = ["set", "update", "set_unconditionally"];

const PRIMITIVE_USERS: [&str; 18] = [
    "crates/beui/",
    "crates/beui-view/",
    "crates/beui-components-styled/",
    "crates/beui-components-unstyled/",
    "crates/beui-dock/",
    "crates/beui-inspector/",
    "crates/beui-keyboard/",
    "crates/beui-renderer-wgpu/",
    "crates/beui-demo/src/base_pages.rs",
    "crates\\beui\\",
    "crates\\beui-view\\",
    "crates\\beui-components-styled\\",
    "crates\\beui-components-unstyled\\",
    "crates\\beui-dock\\",
    "crates\\beui-inspector\\",
    "crates\\beui-keyboard\\",
    "crates\\beui-renderer-wgpu\\",
    "crates\\beui-demo\\src\\base_pages.rs",
];

const COMPONENT_CRATES: [&str; 8] = [
    "crates/beui-components-styled/",
    "crates/beui-components-unstyled/",
    "crates/beui-dock/",
    "crates/beui-keyboard/",
    "crates\\beui-components-styled\\",
    "crates\\beui-components-unstyled\\",
    "crates\\beui-dock\\",
    "crates\\beui-keyboard\\",
];

const BASE_CRATES: [&str; 4] = [
    "crates/beui-core/",
    "crates/beui-types/",
    "crates\\beui-core\\",
    "crates\\beui-types\\",
];

const DOCUMENT_ACCESS: [&str; 5] = [
    "with_document",
    "try_with_document",
    "NodeOf",
    "set_component_state",
    "component_state",
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
    for call in tree.syntax().descendants().filter_map(ast::CallExpr::cast) {
        if test_source || !(calls(&call, "with_document") || calls(&call, "try_with_document")) {
            continue;
        }
        let Some(arguments) = call.arg_list() else {
            continue;
        };
        for read in arguments
            .syntax()
            .descendants()
            .filter_map(ast::MethodCallExpr::cast)
        {
            let closures = read
                .syntax()
                .ancestors()
                .take_while(|node| node != arguments.syntax())
                .filter(|node| ast::ClosureExpr::can_cast(node.kind()))
                .count();
            if closures <= 1
                && read.name_ref().is_some_and(|name| name.text() == "get")
                && read
                    .arg_list()
                    .is_some_and(|arguments| arguments.args().next().is_none())
            {
                violations.push(format!(
                    "signal read under the document: {} read signals before with_document, since a memo they refresh may need the document itself",
                    at(read.syntax())
                ));
            }
        }
    }
    if !PRIMITIVE_USERS
        .iter()
        .any(|allowed| relative.starts_with(allowed))
    {
        violations.extend(primitive_uses(tree, source, relative));
        if !BASE_CRATES.iter().any(|base| relative.starts_with(base)) {
            violations.extend(fixed_extent_uses(tree, source, relative));
        }
    }
    if !test_source {
        violations.extend(component_uses(tree, source, relative));
    }
    if !test_source
        && COMPONENT_CRATES
            .iter()
            .any(|component| relative.starts_with(component))
    {
        violations.extend(document_uses(tree, source, relative));
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
        if attribute_named(&function, "component") {
            let block = function.syntax().parent().and_then(|list| list.parent());
            let unmarked = block.is_some_and(|block| {
                ast::Trait::cast(block.clone())
                    .is_some_and(|block| !attribute_named(&block, "component"))
                    || ast::Impl::cast(block)
                        .is_some_and(|block| !attribute_named(&block, "component"))
            });
            if unmarked {
                let offset = u32::from(function.syntax().text_range().start()) as usize;
                let line = source[..offset].matches('\n').count() + 1;
                violations.push(format!("component method outside a component block: {relative}:{line} a #[component] method needs #[component] on its trait or impl block too"));
            }
            continue;
        }
        if attribute_named(&function, "test") {
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
                "Render"
                    | "RenderFn"
                    | "MenuStyle"
                    | "AnchoredStyle"
                    | "ScrollbarStyle"
                    | "CompletionMenu"
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

fn fixed_extent_uses(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let tokens: Vec<_> = tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && !matches!(token.text(), ":" | "::"))
        .collect();
    tokens
        .windows(2)
        .filter(|pair| pair[0].text() == "Extent" && pair[1].text() == "px")
        .map(|pair| {
            let offset = u32::from(pair[1].text_range().start()) as usize;
            let line = source[..offset].matches('\n').count() + 1;
            format!(
                "fixed extent outside beui: {relative}:{line} a styled length takes a Size for a fixed length (Size::Custom for one that comes from content); keep Extent for intrinsic, fill, fraction and bounds"
            )
        })
        .collect()
}

fn document_uses(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let tokens: Vec<_> = tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && !matches!(token.text(), ":" | "::"))
        .collect();
    tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| {
            DOCUMENT_ACCESS.contains(&token.text())
                || (token.text() == "Document"
                    && index
                        .checked_sub(1)
                        .is_some_and(|before| tokens[before].text() == "document"))
        })
        .map(|(_, token)| {
            let offset = u32::from(token.text_range().start()) as usize;
            let line = source[..offset].matches('\n').count() + 1;
            format!(
                "document in a component: {relative}:{line} components reach the tree through beui::reactive's node functions and test helpers through Inspect, never through Document"
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
