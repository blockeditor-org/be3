use super::*;

#[test]
fn installing_installs_only_what_the_build_produced_and_refuses_a_partial_build() {
    let root = scratch("install-list");
    let source = root.join("Downloads");
    let layout = layout(&root);
    build(&source);
    std::fs::write(source.join("notes.txt"), "someone's notes").unwrap();
    std::fs::create_dir_all(source.join("photos")).unwrap();
    std::fs::write(source.join("photos/cat.jpg"), "a cat").unwrap();

    let receipt = install_into(&source, &layout);
    let home = layout.home();
    let mut installed: Vec<PathBuf> = Vec::new();
    let mut dirs = vec![home.clone()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            match path.is_dir() {
                true => dirs.push(path),
                false => installed.push(path.strip_prefix(&home).unwrap().to_path_buf()),
            }
        }
    }
    installed.sort();
    let mut expected = receipt.files.clone();
    expected.push(PathBuf::from(RECEIPT));
    expected.sort();
    assert_eq!(
        installed, expected,
        "the files the build listed, its list and the receipt, and nothing that only sat beside be-host"
    );

    let refusal = |source: &Path| plan_install(source, &layout).map(|_| ()).unwrap_err();
    std::fs::remove_file(source.join("data/notes/files/readme.txt")).unwrap();
    assert!(
        refusal(&source).contains("is missing data/notes/files/readme.txt"),
        "{}",
        refusal(&source)
    );

    let lone = root.join("lone");
    std::fs::create_dir_all(&lone).unwrap();
    std::fs::write(lone.join(EXECUTABLE), "be-host").unwrap();
    assert!(
        refusal(&lone).contains("has no program-files.txt"),
        "a be-host without the list of what its build produced is refused: {}",
        refusal(&lone)
    );
    std::fs::write(lone.join(LIST), "be-host\n").unwrap();
    assert!(
        refusal(&lone).contains("has no apps.json"),
        "a be-host built without its apps is refused: {}",
        refusal(&lone)
    );
    std::fs::write(lone.join(LIST), "be-host\napps.json\n").unwrap();
    std::fs::write(lone.join("apps.json"), "[\"notes.app.json\"]").unwrap();
    assert!(refusal(&lone).contains("lists the app notes.app.json, which is not there"));
    std::fs::write(lone.join(LIST), "be-host\n../be-host\n").unwrap();
    assert!(refusal(&lone).contains("which is not inside it"));
    std::fs::remove_dir_all(root).unwrap();
}
