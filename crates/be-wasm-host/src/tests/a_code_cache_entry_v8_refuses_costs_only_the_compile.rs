use super::*;

const ADDS: &str = r#"(module
    (func (export "add") (param i32 i32) (result i32)
        (i32.add (local.get 0) (local.get 1)))
)"#;

#[test]
fn a_code_cache_entry_v8_refuses_costs_only_the_compile() {
    let bytes = wat::parse_str(ADDS).unwrap();
    let engine::Isolate {
        shared: isolate,
        context,
        ..
    } = engine::isolate();
    let mut locker = isolate.lock();
    v8::scope!(let scope, &mut *locker);
    let local = v8::Local::new(scope, &context);
    let scope = &mut v8::ContextScope::new(scope, local);

    let module = cache::compile_with(scope, &bytes, b"not a serialized module".to_vec())
        .expect("a corrupt entry is refused and the bytes compiled");
    let compiled = module.get_compiled_module();
    assert_eq!(
        compiled.get_wire_bytes_ref(),
        bytes.as_slice(),
        "the module is the one its bytes make"
    );
    // Nothing has run, so nothing is optimised: V8 serializes only TurboFan code.
    assert_eq!(cache::serialized(&compiled), None);
}
