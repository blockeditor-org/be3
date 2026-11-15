// V8 is linked from the prebuilt library third-party/v8 pins (step 2 of
// plans/wasm_on_v8.md): it compiles and runs a module that adds.
#[test]
#[cfg(not(target_os = "android"))]
fn v8_runs_a_wasm_module() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    let isolate = &mut v8::Isolate::new(Default::default());
    v8::scope!(let scope, isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let module = wat::parse_str(
        r#"(module (func (export "add") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))"#,
    )
    .unwrap();
    let module = v8::WasmModuleObject::compile(scope, &module).unwrap();
    let global = context.global(scope);
    let key = v8::String::new(scope, "module").unwrap();
    global.set(scope, key.into(), module.into()).unwrap();
    let source = v8::String::new(
        scope,
        "new WebAssembly.Instance(module).exports.add(40, 2)",
    )
    .unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    let result = script.run(scope).unwrap();
    assert_eq!(result.int32_value(scope), Some(42));
}
