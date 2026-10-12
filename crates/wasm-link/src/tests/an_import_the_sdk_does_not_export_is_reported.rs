use super::*;

fn import(module: &str, name: &str, kind: u8, rest: &[u8]) -> Vec<u8> {
    let mut entry = vec![module.len() as u8];
    entry.extend_from_slice(module.as_bytes());
    entry.push(name.len() as u8);
    entry.extend_from_slice(name.as_bytes());
    entry.push(kind);
    entry.extend_from_slice(rest);
    entry
}

#[test]
fn an_import_the_sdk_does_not_export_is_reported() {
    let mut imports = vec![4];
    imports.extend(import("env", "sdk_function", module::FUNCTION, &[0]));
    imports.extend(import("env", "missing", module::FUNCTION, &[0]));
    imports.extend(import("GOT.mem", "SDK_DATA", module::GLOBAL_KIND, &[0x7f, 1]));
    imports.extend(import("env", "__memory_base", module::GLOBAL_KIND, &[0x7f, 0]));
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.extend([1, 4, 1, 0x60, 0, 0]);
    bytes.push(2);
    bytes.push(imports.len() as u8);
    bytes.extend(imports);
    assert_eq!(unresolved(&bytes, &sdk(&[])).unwrap(), ["env.missing"]);
}
