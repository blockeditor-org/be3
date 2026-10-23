use super::super::lines::{Kind, kind};

#[test]
fn each_changed_file_counts_toward_one_kind_of_line() {
    for (path, expected) in [
        ("crates/beui/src/layout.rs", Some(Kind::Rust)),
        ("crates/beui/src/tests.rs", Some(Kind::Tests)),
        ("crates/beui/src/tests/a_row_fits.rs", Some(Kind::Tests)),
        ("crates/app-tests/src/lib.rs", Some(Kind::Tests)),
        ("compile_fail/src/view.rs", Some(Kind::Tests)),
        ("crates/local-tools/src/ci.rs", Some(Kind::Build)),
        ("crates/verify/src/lib.rs", Some(Kind::Build)),
        ("crates/local-tools/src/ci/tests.rs", Some(Kind::Tests)),
        ("crates/beui/BUCK", Some(Kind::Build)),
        ("crates/beui/Cargo.toml", Some(Kind::Build)),
        ("buck/dev/verify.bxl", Some(Kind::Build)),
        ("scripts/verify", Some(Kind::Build)),
        ("Cargo.lock", Some(Kind::Build)),
        ("guides/beui.md", Some(Kind::Markdown)),
        ("crates/local-tools/README.md", Some(Kind::Markdown)),
        ("crates/block-app/web/index.js", Some(Kind::Other)),
        ("crates/editors/pdf/manifest.json", Some(Kind::Other)),
        ("scriptsx/run", Some(Kind::Other)),
        ("snapshots/beui-demo.buttons.paint", None),
    ] {
        assert_eq!(kind(path), expected, "{path}");
    }
}
