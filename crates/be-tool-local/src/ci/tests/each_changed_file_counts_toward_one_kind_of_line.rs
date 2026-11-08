use super::super::lines::{Kind, kind};

#[test]
fn each_changed_file_counts_toward_one_kind_of_line() {
    for (path, expected) in [
        ("crates/beui/src/layout.rs", Some(Kind::Rust)),
        ("crates/beui/src/tests.rs", Some(Kind::Tests)),
        ("crates/beui/src/tests/a_row_fits.rs", Some(Kind::Tests)),
        ("crates/be-host-tests/src/lib.rs", Some(Kind::Tests)),
        ("compile_fail/src/view.rs", Some(Kind::Tests)),
        ("crates/be-tool-local/src/ci.rs", Some(Kind::Build)),
        ("crates/be-tool-verify/src/lib.rs", Some(Kind::Build)),
        ("crates/be-tool-local/src/ci/tests.rs", Some(Kind::Tests)),
        ("crates/beui/BUCK", Some(Kind::Build)),
        ("crates/beui/Cargo.toml", Some(Kind::Build)),
        ("buck/dev/test.bxl", Some(Kind::Build)),
        ("scripts/verify", Some(Kind::Build)),
        ("Cargo.lock", Some(Kind::Build)),
        ("guides/beui.md", Some(Kind::Markdown)),
        ("crates/be-tool-local/README.md", Some(Kind::Markdown)),
        ("crates/be-host/web/index.js", Some(Kind::Other)),
        ("crates/be-app-pdf/manifest.json", Some(Kind::Other)),
        ("scriptsx/run", Some(Kind::Other)),
        ("crates/beui-demo/snapshots/buttons.paint", None),
        (
            "crates/be-app-checklist/snapshots/adding_an_item_puts_it_on_the_list.paint",
            None,
        ),
    ] {
        assert_eq!(kind(path), expected, "{path}");
    }
}
