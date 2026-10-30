use super::*;

#[test]
fn a_symbol_two_libraries_define_is_refused() {
    let bytes = importing(&[("env", "shared", module::FUNCTION)]);
    let closure = [
        library("left", &[], &[], &["shared"]),
        library("right", &[], &[], &["shared"]),
    ];
    let error = unresolved(&bytes, "plugin", &closure).unwrap_err();
    assert!(error.contains("env.shared (in left and right)"), "{error}");
    assert_eq!(unresolved(&bytes, "plugin", &closure[..1]), Ok(()));
}
