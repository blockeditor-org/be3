use crate::stage::layout;

#[test]
fn stage_compiled_only_keeps_a_module_only_without_its_cwasm() {
    let directory = std::env::temp_dir().join(format!("stage-compiled-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = |name: &str| directory.join(name).to_string_lossy().into_owned();
    std::fs::write(path("a.wasm"), "a").unwrap();
    std::fs::write(path("b.wasm"), "b").unwrap();
    let staged = layout(&[
        "--compiled-only".into(),
        format!("--library={}", path("a.wasm")),
        format!("--library-artifact={}", path("a.wasm")),
        format!("--library={}", path("b.wasm")),
    ])
    .unwrap();
    let names: Vec<_> = staged
        .keys()
        .map(|name| format!("{}{}", &name[..8], &name[name.find('.').unwrap()..]))
        .collect();
    // b's module, which has no .cwasm, and a's .cwasm alone.
    assert_eq!(names, ["lib-3e23.wasm", "lib-ca97.cwasm"]);
    let _ = std::fs::remove_dir_all(directory);
}
