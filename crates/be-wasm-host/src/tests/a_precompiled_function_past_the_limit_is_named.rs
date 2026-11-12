use super::*;

#[test]
fn a_precompiled_function_past_the_limit_is_named() {
    let body = "(drop (i32.const 1)) (call $small)".repeat(500);
    let wat = format!("(module (func $small) (func $big (export \"big\") {body}))");
    let compiled = wasmtime::Engine::default()
        .precompile_module(wat.as_bytes())
        .unwrap();
    assert_eq!(oversized(&compiled, LARGEST_FUNCTION), Ok(Vec::new()));
    let found = oversized(&compiled, 1024).unwrap();
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["wasm[0]::function[1]::big"]);
    assert!(found[0].1 > 1024);
    assert!(oversized(b"not an object", 1024).is_err());
}
