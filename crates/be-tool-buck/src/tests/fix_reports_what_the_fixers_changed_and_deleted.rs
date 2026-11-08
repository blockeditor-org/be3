#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn fix_reports_what_the_fixers_changed_and_deleted() {
    let directory = std::env::temp_dir().join(format!("fix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let sources = directory.join("sources");
    std::fs::create_dir_all(sources.join("crates/a/src")).unwrap();
    std::fs::write(sources.join("crates/a/src/lib.rs"), "fn a() {}\n").unwrap();
    std::fs::write(sources.join("crates/a/src/gone.rs"), "gone\n").unwrap();
    std::fs::write(sources.join("crates/a/src/same.rs"), "same\n").unwrap();
    std::fs::write(sources.join("crates/a/BUCK"), "cargo_crate()\n").unwrap();
    // A fix-rust-source that removes a file and adds one, and a rustfmt and a
    // starlark_fmt that each append to the files they are given.
    script(
        &directory.join("fix-rust-source"),
        "rm crates/a/src/gone.rs && echo new > crates/a/src/new.rs",
    );
    script(
        &directory.join("rustfmt/bin/rustfmt"),
        "shift 2; for file; do case $file in *lib.rs) echo '// formatted' >> \"$file\";; esac; done",
    );
    script(
        &directory.join("starlark_fmt"),
        "shift 3; for file; do echo '# formatted' >> \"$file\"; done",
    );
    std::fs::write(directory.join("config.json"), "{}").unwrap();
    let out = directory.join("out");
    let arguments: Vec<String> = [
        out.as_path(),
        &sources,
        Path::new("--fix-rust-source"),
        &directory.join("fix-rust-source"),
        Path::new("--rustfmt"),
        &directory.join("rustfmt"),
        Path::new("--starlark-fmt"),
        &directory.join("starlark_fmt"),
        Path::new("--starlark-config"),
        &directory.join("config.json"),
    ]
    .iter()
    .map(|path| path.to_string_lossy().into_owned())
    .collect();
    crate::fix::run(&arguments).unwrap();
    let read = |path: &str| std::fs::read_to_string(out.join(path)).unwrap();
    assert_eq!(read("deleted"), "crates/a/src/gone.rs\n");
    assert_eq!(read("original/crates/a/src/gone.rs"), "gone\n");
    assert_eq!(
        read("changed/crates/a/src/lib.rs"),
        "fn a() {}\n// formatted\n"
    );
    assert_eq!(read("original/crates/a/src/lib.rs"), "fn a() {}\n");
    assert_eq!(read("changed/crates/a/src/new.rs"), "new\n");
    assert!(!out.join("original/crates/a/src/new.rs").exists());
    assert_eq!(
        read("changed/crates/a/BUCK"),
        "cargo_crate()\n# formatted\n"
    );
    assert!(!out.join("changed/crates/a/src/same.rs").exists());
    let _ = std::fs::remove_dir_all(directory);
}
