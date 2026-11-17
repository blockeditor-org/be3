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
    let unchanged = wat::parse_str(r#"(module (func (export "app_start")))"#).unwrap();
    assert_eq!(engine::importing_memory(&unchanged).unwrap(), unchanged);
}
