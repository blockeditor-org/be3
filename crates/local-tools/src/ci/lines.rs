use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Rust,
    Tests,
    NewTests,
    Build,
    Markdown,
    Other,
}

const KINDS: [(Kind, &str); 6] = [
    (Kind::Rust, "Rust (excluding tests and build tooling)"),
    (Kind::NewTests, "new tests"),
    (Kind::Tests, "existing tests"),
    (Kind::Build, "build system"),
    (Kind::Markdown, "Markdown"),
    (Kind::Other, "everything else"),
];

const TOOLING: [&str; 4] = [
    "crates/local-tools",
    "crates/verify",
    "crates/fix-rust-source",
    "crates/buck-tools",
];

const BUILD_DIRECTORIES: [&str; 4] = ["scripts", "buck", ".github/workflows", ".github/actions"];

const BUILD_FILES: [&str; 9] = [
    "BUCK.v2",
    ".github/zizmor.yml",
    ".cargo/config.toml",
    ".config/nextest.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "clippy.toml",
    ".buckconfig",
];

pub fn kind(path: &str) -> Option<Kind> {
    let under = |directory: &str| {
        path.strip_prefix(directory)
            .is_some_and(|rest| rest.starts_with('/'))
    };
    let name = path.rsplit('/').next().unwrap_or(path);
    if under("snapshots") {
        return None;
    }
    if name.ends_with(".md") {
        return Some(Kind::Markdown);
    }
    let rust = name.ends_with(".rs");
    let test = under("compile_fail")
        || under("crates/app-tests")
        || (rust && (name == "tests.rs" || path.split('/').any(|part| part == "tests")));
    if test {
        return Some(Kind::Tests);
    }
    let build = TOOLING.iter().any(|directory| under(directory))
        || BUILD_DIRECTORIES.iter().any(|directory| under(directory))
        || BUILD_FILES.contains(&path)
        || ["BUCK", "PACKAGE", "Cargo.toml"].contains(&name)
        || [".bzl", ".bxl", ".sh"]
            .iter()
            .any(|extension| name.ends_with(extension));
    if build {
        return Some(Kind::Build);
    }
    if rust && under("crates") {
        return Some(Kind::Rust);
    }
    Some(Kind::Other)
}

pub fn summary(numstat: &str, added: &[&str]) -> String {
    let mut totals = [(0u64, 0u64, 0u64); KINDS.len()];
    for line in numstat.lines() {
        let mut fields = line.splitn(3, '\t');
        let (Some(plus), Some(minus), Some(path)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Ok(plus), Ok(minus)) = (plus.parse::<u64>(), minus.parse::<u64>()) else {
            continue;
        };
        let Some(kind) = kind(path) else {
            continue;
        };
        let kind = match kind {
            Kind::Tests if added.contains(&path) => Kind::NewTests,
            kind => kind,
        };
        let Some(index) = KINDS.iter().position(|(each, _)| *each == kind) else {
            continue;
        };
        let total = &mut totals[index];
        total.0 += plus;
        total.1 += minus;
        total.2 += 1;
    }
    let mut text = String::new();
    for ((_, label), (added, removed, files)) in KINDS.iter().zip(totals) {
        let _ = writeln!(
            text,
            "**+{added} -{removed}** of {label} in {files} {}.\n",
            if files == 1 { "file" } else { "files" }
        );
    }
    text
}
