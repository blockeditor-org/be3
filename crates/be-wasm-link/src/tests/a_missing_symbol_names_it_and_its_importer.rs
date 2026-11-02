use super::*;

#[test]
fn a_missing_symbol_names_it_and_its_importer() {
    let bytes = importing(&[
        ("env", "base_function", module::FUNCTION),
        ("env", "missing", module::FUNCTION),
        ("GOT.mem", "BASE_DATA", module::GLOBAL_KIND),
        ("GOT.tls", "BASE_LOCAL", module::GLOBAL_KIND),
        ("GOT.tls", "BASE_DATA", module::GLOBAL_KIND),
        ("env", "__memory_base", module::GLOBAL_KIND),
    ]);
    let closure = [library("base", &[], &[], &["base_function"])];
    let error = unresolved(&bytes, "triangle", &closure).unwrap_err();
    assert!(error.starts_with("triangle imports 2 symbols"), "{error}");
    assert!(
        error.contains("env.missing") && error.contains("GOT.tls.BASE_DATA"),
        "{error}"
    );
    assert!(!error.contains("base_function"), "{error}");
}
