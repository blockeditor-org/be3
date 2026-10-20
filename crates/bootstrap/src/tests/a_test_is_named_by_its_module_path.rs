use super::*;

#[test]
fn a_test_is_named_by_its_module_path() {
    let crate_of = |file, function, name| crate_of_function(file, function, name);
    assert_eq!(
        crate_of(
            "crates/block-app/src/media/tests/plays.rs",
            "plays",
            "media::tests::plays::plays"
        ),
        Some("crates/block-app".to_owned())
    );
    assert_eq!(
        crate_of("crates/beui/src/lib.rs", "lays_out", "lays"),
        Some("crates/beui".to_owned())
    );
    assert_eq!(
        crate_of("crates/beui/src/text/mod.rs", "wraps", "text::wraps"),
        Some("crates/beui".to_owned())
    );
    assert_eq!(
        crate_of("crates/beui/src/bin/tool/main.rs", "runs", "main::runs"),
        None
    );
    assert_eq!(
        crate_of("crates/beui/tests/layout.rs", "fits", "fits"),
        Some("crates/beui".to_owned())
    );
    assert_eq!(
        crate_of("crates/beui/tests/layout.rs", "fits", "layout::fits"),
        None
    );
    assert_eq!(crate_of("crates/beui/build.rs", "fits", "fits"), None);
}
