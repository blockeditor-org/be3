use ra_ap_syntax::ast::{self, AstNode};
use ra_ap_syntax::{SourceFile, SyntaxNode};

const STABLE_CRATES: [&str; 32] = [
    "reactive",
    "beui-tree",
    "beui-types",
    "beui-core",
    "beui-core-html",
    "beui-view",
    "beui-html",
    "beui-components-unstyled",
    "beui-components-styled",
    "beui-dock",
    "beui-host",
    "beui-inspector",
    "beui-layer-drive",
    "beui-layer-space",
    "beui-layer-xr",
    "beui-platform-drm",
    "beui-platform-plugin",
    "beui-platform-plugin-api",
    "beui-platform-winit",
    "beui-platform-web",
    "beui-platform-headless",
    "beui-platform-android",
    "beui-renderer-wgpu",
    "beui-font-freetype",
    "beui-font-browser",
    "beui-demo",
    "beui-runner",
    "block-app-plugin-host",
    "block-editor-plugin",
    "block-editor-beui",
    "block-plugin-api",
    "block-ui",
];

const RANDOM: [&str; 13] = [
    "std::collections::HashMap",
    "std::collections::HashSet",
    "std::collections::hash_map::HashMap",
    "std::collections::hash_set::HashSet",
    "std::collections::hash_map::RandomState",
    "std::hash::RandomState",
    "collections::HashMap",
    "collections::HashSet",
    "foldhash::HashMap",
    "foldhash::HashSet",
    "foldhash::fast::RandomState",
    "foldhash::quality::RandomState",
    "hashbrown::HashMap",
];

pub(crate) fn hash_order_violations(
    tree: &SourceFile,
    source: &str,
    relative: &str,
) -> Vec<String> {
    let normalized = relative.replace('\\', "/");
    if !STABLE_CRATES
        .iter()
        .any(|name| normalized.starts_with(&format!("crates/{name}/")))
    {
        return Vec::new();
    }
    let at = |node: &SyntaxNode| {
        let offset = u32::from(node.text_range().start()) as usize;
        source[..offset].matches('\n').count() + 1
    };
    let mut found: Vec<(usize, String)> = Vec::new();
    for import in tree.syntax().descendants().filter_map(ast::Use::cast) {
        for leaf in crate::use_leaves(&import) {
            let path = leaf.split(' ').next().unwrap_or_default();
            if random(path) {
                found.push((at(import.syntax()), path.to_owned()));
            }
        }
    }
    for path in tree.syntax().descendants().filter_map(ast::Path::cast) {
        if path.syntax().parent().and_then(ast::Path::cast).is_some()
            || path
                .syntax()
                .ancestors()
                .any(|node| ast::Use::can_cast(node.kind()))
        {
            continue;
        }
        let written = without_generics(&path.syntax().text().to_string());
        if random(&written) {
            found.push((at(path.syntax()), written));
        }
    }
    found
        .into_iter()
        .map(|(line, path)| {
            format!(
                "randomly seeded hash map: {normalized}:{line} {path} iterates in a different order on every run, and so does whatever is built from it; use stable_hash::HashMap or stable_hash::HashSet"
            )
        })
        .collect()
}

fn random(path: &str) -> bool {
    let path = path.trim_start_matches("::");
    RANDOM.iter().any(|random| {
        path.strip_prefix(random)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
    })
}

fn without_generics(text: &str) -> String {
    let mut depth = 0usize;
    let mut kept = String::new();
    for character in text.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 && !character.is_whitespace() => kept.push(character),
            _ => {}
        }
    }
    kept.replace("::::", "::")
}
