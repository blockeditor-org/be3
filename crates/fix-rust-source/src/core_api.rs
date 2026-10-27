use ra_ap_syntax::ast::{self, AstNode, HasAttrs, HasModuleItem, HasName, HasVisibility};
use ra_ap_syntax::{SourceFile, SyntaxNode};

pub(crate) const CORE_FILES: [&str; 8] = [
    "crates/beui-view/src/native/",
    "crates/beui-view/src/native.rs",
    "crates/beui-view/src/html/",
    "crates/beui-view/src/html.rs",
    "crates\\beui-view\\src\\native\\",
    "crates\\beui-view\\src\\native.rs",
    "crates\\beui-view\\src\\html\\",
    "crates\\beui-view\\src\\html.rs",
];

const FACADES: [&str; 4] = [
    "crates/beui/src/",
    "crates/beui-view/src/",
    "crates\\beui\\src\\",
    "crates\\beui-view\\src\\",
];

const ONE_CORE_MODULES: [(&str, &str, &str); 4] = [
    (
        "crates/beui/src/lib.rs",
        "native",
        "the native runtime: apps, platforms, runners, documents, renderers, fonts and the native-only escapes",
    ),
    (
        "crates/beui/src/lib.rs",
        "html",
        "running, hydrating and server-rendering a page on the HTML core",
    ),
    (
        "crates/beui-view/src/lib.rs",
        "native_api",
        "what beui::native needs from beui-view: GPU drawings and the native painter behind a Painter",
    ),
    (
        "crates/beui-view/src/lib.rs",
        "running",
        "the HTML core's runners, which beui::html re-exports",
    ),
];

pub(crate) fn core_api_violations(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    if !FACADES.iter().any(|facade| relative.starts_with(facade))
        || relative.contains("/tests/")
        || relative.ends_with("/tests.rs")
        || relative.contains("\\tests\\")
        || relative.ends_with("\\tests.rs")
    {
        return Vec::new();
    }
    let core_file = CORE_FILES.iter().any(|file| relative.starts_with(file));
    let mut violations = Vec::new();
    visit(
        tree.items(),
        core_file,
        false,
        source,
        relative,
        &mut violations,
    );
    violations
}

fn visit(
    items: impl Iterator<Item = ast::Item>,
    core_file: bool,
    chosen: bool,
    source: &str,
    relative: &str,
    violations: &mut Vec<String>,
) {
    for item in items {
        let chosen = chosen || chooses_core(&item);
        let name = item_name(&item);
        let normalized = relative.replace('\\', "/");
        if ONE_CORE_MODULES.iter().any(|(file, allowed, _)| {
            *file == normalized && exported(&item).as_deref() == Some(*allowed)
        }) {
            continue;
        }
        if let ast::Item::Module(module) = &item {
            if let Some(list) = module.item_list() {
                visit(
                    list.items(),
                    core_file,
                    chosen,
                    source,
                    relative,
                    violations,
                );
            }
        }
        if !(core_file || chosen) || !unrestricted(&item) {
            continue;
        }
        let line = line(source, item.syntax());
        let name = name.unwrap_or_else(|| "this item".to_owned());
        violations.push(match core_file {
            true => format!(
                "public item in a core: {relative}:{line} `{name}` is public in a per-core module; the cores give only pub(crate) bodies, so declare it once in shared code (beui-types, or beui-view's shared modules with a private per-core field) and call the core from there (guides/beui.md, Cores)"
            ),
            false => format!(
                "one-core public item: {relative}:{line} `{name}` is public under a cfg(beui_html) choice, so code using it compiles on one core only; declare it once in shared code with a body per core, or put it in one of the one-core modules fix-rust-source's core_api.rs lists, with a reason (guides/beui.md, Cores)"
            ),
        });
    }
}

fn chooses_core(item: &ast::Item) -> bool {
    let Some(attributed) = ast::AnyHasAttrs::cast(item.syntax().clone()) else {
        return false;
    };
    attributed.attrs().any(|attribute| {
        matches!(attribute.meta(), Some(ast::Meta::CfgMeta(_)))
            && attribute
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.text() == "beui_html")
    })
}

fn unrestricted(item: &ast::Item) -> bool {
    ast::AnyHasVisibility::cast(item.syntax().clone())
        .and_then(|item| item.visibility())
        .is_some_and(|visibility| visibility.syntax().text() == "pub")
}

fn exported(item: &ast::Item) -> Option<String> {
    if let ast::Item::Use(used) = item {
        let tree = used.use_tree()?;
        if let Some(rename) = tree.rename() {
            return rename.name().map(|name| name.text().to_string());
        }
        return tree
            .path()?
            .segment()?
            .name_ref()
            .map(|name| name.text().to_string());
    }
    item_name(item)
}

fn item_name(item: &ast::Item) -> Option<String> {
    if let ast::Item::Use(used) = item {
        return used.use_tree().map(|tree| tree.syntax().text().to_string());
    }
    ast::AnyHasName::cast(item.syntax().clone())
        .and_then(|named| named.name())
        .map(|name| name.text().to_string())
}

fn line(source: &str, node: &SyntaxNode) -> usize {
    let offset = u32::from(node.text_range().start()) as usize;
    source[..offset].matches('\n').count() + 1
}
