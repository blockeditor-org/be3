use super::*;

#[test]
fn a_module_from_bytes_imports_the_memory_it_defines() {
    let defining = wat::parse_str(
        r#"(module
            (memory (export "memory") 2 5)
            (data (i32.const 0) "kept")
            (func (export "app_start")))"#,
    )
    .unwrap();
    let importing = engine::importing_memory(&defining).unwrap();
    wasmparser::Validator::new()
        .validate_all(&importing)
        .expect("the module is still valid");
    let shape = engine::Shape::read(&importing).unwrap();
    let memory = shape
        .memory_import()
        .expect("the memory is imported instead");
    assert_eq!(
        (memory.module.as_str(), memory.name.as_str(), &memory.kind),
        (
            "env",
            "memory",
            &engine::Kind::Memory(engine::Memory {
                initial: 2,
                maximum: Some(5),
                shared: false,
            })
        )
    );
    // Beside a function it imports, whose index (and so every call and export) stays as it was.
    let merged = engine::importing_memory(
        &wat::parse_str(
            r#"(module
                (import "be3_host" "host_wake" (func $wake))
                (memory 1)
                (func $start (call $wake))
                (export "app_start" (func $start)))"#,
        )
        .unwrap(),
    )
    .unwrap();
    wasmparser::Validator::new().validate_all(&merged).unwrap();
    let shape = engine::Shape::read(&merged).unwrap();
    let imported: Vec<(&str, &str)> = shape
        .imports
        .iter()
        .map(|import| (import.module.as_str(), import.name.as_str()))
        .collect();
    assert_eq!(imported, [("be3_host", "host_wake"), ("env", "memory")]);
    let exported = wasmparser::Parser::new(0)
        .parse_all(&merged)
        .find_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ExportSection(exports) => {
                Some(exports.into_iter().next().unwrap().unwrap().index)
            }
            _ => None,
        });
    assert_eq!(
        exported,
        Some(1),
        "app_start is still the function after the import"
    );
    // A memory and nothing else: the import section goes last.
    let alone = engine::importing_memory(&wat::parse_str("(module (memory 1))").unwrap()).unwrap();
    wasmparser::Validator::new().validate_all(&alone).unwrap();
    assert!(
        engine::Shape::read(&alone)
            .unwrap()
            .memory_import()
            .is_some()
    );
    let unchanged = wat::parse_str(r#"(module (func (export "app_start")))"#).unwrap();
    assert_eq!(engine::importing_memory(&unchanged).unwrap(), unchanged);
}
