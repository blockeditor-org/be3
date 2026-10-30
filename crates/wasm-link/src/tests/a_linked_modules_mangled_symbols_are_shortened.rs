use super::*;

#[test]
fn a_linked_modules_mangled_symbols_are_shortened() {
    let mangled = "_RNvNtCs4fqI2P2rA04_8reactive6memo11create_memo";
    let constant = "anon.f0eed36672583d82a5d5643640160405.1.llvm.4702199943595589468";
    let bytes = importing(&[
        (ENV, mangled, module::FUNCTION),
        (GOT_MEMORY, constant, module::GLOBAL_KIND),
        (ENV, "__stack_pointer", module::GLOBAL_KIND),
        ("wasi_snapshot_preview1", "_R_a_wasi_name_that_is_long_enough", module::FUNCTION),
    ]);
    let bytes =
        module::with_exported_globals(&bytes, &[(mangled.to_owned(), 0), ("plain".to_owned(), 1)])
            .unwrap();
    let short = module::with_short_symbols(&bytes).unwrap();
    let imports: Vec<(String, String)> = module::imports(&short)
        .unwrap()
        .into_iter()
        .map(|import| (import.module, import.name))
        .collect();
    assert_eq!(
        imports,
        [
            (ENV.to_owned(), wasm_dylink::short_symbol(mangled).into_owned()),
            (GOT_MEMORY.to_owned(), wasm_dylink::short_symbol(constant).into_owned()),
            (ENV.to_owned(), "__stack_pointer".to_owned()),
            (
                "wasi_snapshot_preview1".to_owned(),
                "_R_a_wasi_name_that_is_long_enough".to_owned()
            ),
        ]
    );
    let exports: Vec<String> = module::exports(&short)
        .unwrap()
        .into_iter()
        .map(|export| export.name)
        .collect();
    assert_eq!(
        exports,
        [wasm_dylink::short_symbol(mangled).into_owned(), "plain".to_owned()]
    );
    assert_eq!(module::exported_constant(&short, "plain"), Ok(Some(1)));
    assert!(short.len() < bytes.len());
}
