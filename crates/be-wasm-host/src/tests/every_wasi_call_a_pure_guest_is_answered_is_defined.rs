use super::*;

/// A guest importing every call be-wasm-pure-wasi answers, by its signature there, and nothing
/// else of WASI.
fn importing_all() -> String {
    let imports: String = be_wasm_pure_wasi::ANSWERED
        .iter()
        .map(|name| {
            let parameters = be_wasm_pure_wasi::parameters(name)
                .unwrap_or_else(|| panic!("{name} has no signature"))
                .iter()
                .map(|word| match word {
                    be_wasm_pure_wasi::Word::I32 => " i32",
                    be_wasm_pure_wasi::Word::I64 => " i64",
                })
                .collect::<String>();
            let result = match be_wasm_pure_wasi::returns_errno(name) {
                true => "(result i32)",
                false => "",
            };
            format!(r#"(import "{}" "{name}" (func (param{parameters}) {result}))"#, be_wasm_pure_wasi::MODULE)
        })
        .collect();
    format!(
        r#"(module {imports}
            (memory (export "memory") 1)
            (func (export "app_start"))
            (func (export "app_step"))
            (func (export "app_shutdown")))"#
    )
}

#[test]
fn every_wasi_call_a_pure_guest_is_answered_is_defined() {
    Host::pure()
        .unwrap()
        .load_bytes(importing_all().as_bytes())
        .expect("every answered call is defined");
    // The guest the browser's tests run imports them all too, so guest.js is held to the list.
    let shared = wat::parse_str(include_str!("pure_wasi.wat")).unwrap();
    let mut imported = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&shared) {
        if let wasmparser::Payload::ImportSection(reader) = payload.unwrap() {
            for import in reader.into_imports() {
                imported.push(import.unwrap().name.to_owned());
            }
        }
    }
    for name in be_wasm_pure_wasi::ANSWERED {
        assert!(imported.iter().any(|import| import == name), "pure_wasi.wat leaves out {name}");
    }
}
