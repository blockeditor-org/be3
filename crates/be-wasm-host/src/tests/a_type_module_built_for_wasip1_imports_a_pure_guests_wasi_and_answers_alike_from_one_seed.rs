use super::*;

#[test]
fn a_type_module_built_for_wasip1_imports_a_pure_guests_wasi_and_answers_alike_from_one_seed() {
    for import in wasmparser::Parser::new(0)
        .parse_all(COUNTER_TYPES)
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ImportSection(reader) => Some(reader),
            _ => None,
        })
        .flat_map(|reader| reader.into_imports())
    {
        let import = import.unwrap();
        assert!(
            import.module == "be3_host" || be_wasm_pure_wasi::answers(import.module, import.name),
            "the type module imports {}.{}",
            import.module,
            import.name
        );
    }
    assert_eq!(counter_answers([7; 32]), counter_answers([7; 32]));
}
