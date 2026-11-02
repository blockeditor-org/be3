use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken};
use std::collections::HashSet;

const CATEGORIES: [(&str, &str); 4] = [
    (
        "ffi",
        "unavoidable boundary unsafe: calling C, libc, the kernel, JNI, a wasm host import or a C library; exporting a symbol; unsafe extern blocks; C structs and raw pointers made only for such an interface; unsafe fns a foreign interface requires",
    ),
    ("env", "std::env::set_var or remove_var"),
    (
        "memory",
        "reinterpreting or managing memory no foreign interface requires: transmute, from_raw_parts, get_unchecked, Box::from_raw of our own pointers, unchecked UTF-8, GlobalAlloc, static mut, our own unsafe fns with invariants",
    ),
    ("send-sync", "unsafe impl Send or unsafe impl Sync"),
];

const TAG_PREFIX: &str = "// unsafe(";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsafeSite {
    pub category: Option<String>,
    pub line: usize,
}

pub fn unsafe_sites(source: &str) -> Vec<UnsafeSite> {
    let tree = SourceFile::parse(source, Edition::Edition2024).tree();
    sites(tree.syntax())
        .into_iter()
        .map(|site| UnsafeSite {
            category: nearest_tag(&site)
                .and_then(|tag| tag_category(tag.text()).map(str::to_owned)),
            line: line(source, &site),
        })
        .collect()
}

pub(crate) fn unsafe_violations(tree: &SourceFile, source: &str, relative: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let mut used = HashSet::new();
    for site in sites(tree.syntax()) {
        match nearest_tag(&site) {
            Some(tag) => {
                used.insert(tag.text_range().start());
            }
            None => violations.push(format!(
                "unsafe tag missing: {relative}:{} put a line `// unsafe(CATEGORY)` directly above this unsafe, its statement or match arm, or an enclosing fn, impl, mod or extern block; the nearest tag above wins, so an inner tag overrides an outer one",
                line(source, &site)
            )),
        }
    }
    for token in tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::COMMENT && token.text().starts_with(TAG_PREFIX))
    {
        let at = line(source, &token);
        if !is_tag(&token) {
            violations.push(format!(
                "unsafe tag malformed: {relative}:{at} a tag is a line holding only `// unsafe(CATEGORY)`, with exactly one known CATEGORY and nothing else"
            ));
        } else if !used.contains(&token.text_range().start()) {
            violations.push(format!(
                "unsafe tag stale: {relative}:{at} this tag is not the nearest tag of any unsafe; move it directly above the unsafe it categorizes, or delete it"
            ));
        }
    }
    if !violations.is_empty() {
        let categories = CATEGORIES
            .iter()
            .map(|(name, meaning)| format!("{name}: {meaning}"))
            .collect::<Vec<_>>()
            .join("; ");
        violations.push(format!(
            "unsafe tag categories: every unsafe block, unsafe fn, unsafe impl, unsafe trait, unsafe extern block and #[unsafe(...)] attribute needs one. {categories}. A tag says what kind of unsafe it is, not that it is sound"
        ));
    }
    violations
}

pub(crate) fn kept_tag(token: &SyntaxToken) -> bool {
    token.text().starts_with(TAG_PREFIX) && starts_line(token) && ends_line(token)
}

fn sites(root: &SyntaxNode) -> Vec<SyntaxToken> {
    root.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::UNSAFE_KW)
        .filter(|token| {
            let Some(parent) = token.parent() else {
                return false;
            };
            match parent.kind() {
                SyntaxKind::BLOCK_EXPR
                | SyntaxKind::IMPL
                | SyntaxKind::TRAIT
                | SyntaxKind::EXTERN_BLOCK
                | SyntaxKind::UNSAFE_META => true,
                SyntaxKind::FN => !parent
                    .ancestors()
                    .any(|node| node.kind() == SyntaxKind::EXTERN_ITEM_LIST),
                _ => false,
            }
        })
        .collect()
}

fn nearest_tag(site: &SyntaxToken) -> Option<SyntaxToken> {
    for node in site
        .parent()?
        .ancestors()
        .take_while(|node| node.kind() != SyntaxKind::SOURCE_FILE)
    {
        let Some(first) = node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
        else {
            continue;
        };
        let mut previous = first.prev_token();
        while let Some(token) = previous {
            match token.kind() {
                SyntaxKind::WHITESPACE => {}
                SyntaxKind::COMMENT => {
                    if is_tag(&token) {
                        return Some(token);
                    }
                }
                _ => break,
            }
            previous = token.prev_token();
        }
    }
    None
}

fn is_tag(token: &SyntaxToken) -> bool {
    token.kind() == SyntaxKind::COMMENT
        && tag_category(token.text()).is_some()
        && starts_line(token)
        && ends_line(token)
}

fn tag_category(text: &str) -> Option<&'static str> {
    let category = text.strip_prefix(TAG_PREFIX)?.strip_suffix(')')?;
    CATEGORIES
        .iter()
        .map(|(name, _)| *name)
        .find(|name| *name == category)
}

fn starts_line(token: &SyntaxToken) -> bool {
    token.prev_token().is_none_or(|previous| {
        previous.kind() == SyntaxKind::WHITESPACE
            && (previous.text().contains('\n') || previous.prev_token().is_none())
    })
}

fn ends_line(token: &SyntaxToken) -> bool {
    token
        .next_token()
        .is_none_or(|next| next.kind() == SyntaxKind::WHITESPACE && next.text().contains('\n'))
}

fn line(source: &str, token: &SyntaxToken) -> usize {
    let offset = u32::from(token.text_range().start()) as usize;
    source[..offset.min(source.len())].matches('\n').count() + 1
}
