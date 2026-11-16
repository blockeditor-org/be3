use super::*;

#[test]
fn a_code_cache_entry_that_is_not_whole_or_not_the_modules_is_refused() {
    let directory = std::env::temp_dir().join(format!("code-cache-{}", std::process::id()));
    let module = be_wasm_dylink::identity(b"a module");
    let path = directory.join("version").join(format!("{module}.v8"));
    cache::write(&path, b"what V8 serialized").unwrap();
    assert_eq!(cache::read(&path).as_deref(), Some(&b"what V8 serialized"[..]));

    let whole = std::fs::read(&path).unwrap();
    std::fs::write(&path, &whole[..whole.len() - 1]).unwrap();
    assert_eq!(cache::read(&path), None, "a truncated entry is refused");

    let other = directory
        .join("version")
        .join(format!("{}.v8", be_wasm_dylink::identity(b"another module")));
    std::fs::write(&other, &whole).unwrap();
    assert_eq!(cache::read(&other), None, "an entry under another module's name is refused");
}
