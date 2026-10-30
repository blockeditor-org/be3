use super::*;

#[test]
fn the_libraries_are_read_in_dependency_order() {
    let base = "a".repeat(64);
    let beui = "b".repeat(64);
    let exports = [library_export(1, &beui), "plugin_start".to_owned(), library_export(0, &base)];
    assert_eq!(
        libraries(exports.iter().map(String::as_str)),
        Ok(vec![base.clone(), beui])
    );
    assert_eq!(libraries(["plugin_start"]), Ok(Vec::new()));
    assert!(libraries([library_export(1, &base).as_str()]).is_err());
    assert!(libraries(["__be3_lib_0_nothex"]).is_err());
    assert_eq!(library_file_name(&base), format!("lib-{base}.wasm"));
}
