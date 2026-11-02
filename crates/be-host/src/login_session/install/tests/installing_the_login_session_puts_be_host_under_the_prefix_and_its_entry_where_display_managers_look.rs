use super::*;

#[test]
fn installing_the_login_session_puts_be_host_under_the_prefix_and_its_entry_where_display_managers_look()
 {
    let root = scratch("install");
    let source = root.join("app");
    let layout = layout(&root);
    let prefix = layout.prefix.clone();
    let old = prefix.join("share/wayland-sessions/be-host.desktop");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "an older entry").unwrap();
    build(&source);
    let stale = prefix.join("lib/be-host/stale.app.json");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(
        prefix.join("lib/be-host").join(EXECUTABLE),
        "an older be-host",
    )
    .unwrap();
    std::fs::write(&stale, "{}").unwrap();

    let receipt = install_into(&source, &layout);
    assert_eq!(install_into(&source, &layout), receipt);

    let home = prefix.join("lib/be-host");
    assert_eq!(
        std::fs::read_to_string(home.join(EXECUTABLE)).unwrap(),
        "be-host"
    );
    assert!(home.join("notes.app.json").is_file());
    assert_eq!(
        std::fs::read_to_string(home.join("data/notes/files/readme.txt")).unwrap(),
        "hello"
    );
    assert!(!stale.exists(), "an app left from an older install is gone");
    assert!(!home.with_extension("new").exists());
    assert!(!home.with_extension("old").exists());
    assert_eq!(
        std::fs::read_link(prefix.join("bin/be-host")).unwrap(),
        home.join(EXECUTABLE)
    );
    let entries: Vec<PathBuf> = layout
        .sessions
        .iter()
        .map(|dir| dir.join("be-host.desktop"))
        .collect();
    assert_eq!(receipt.entries, entries);
    for entry in entries {
        assert_eq!(
            std::fs::read_to_string(entry).unwrap(),
            session_entry(&home.join(EXECUTABLE))
        );
    }
    assert!(
        !old.exists(),
        "an entry under the prefix, which LightDM never reads, is gone"
    );
    assert_eq!(
        Receipt::read(&home),
        Some(receipt),
        "the install records what it placed, for uninstalling"
    );
    std::fs::remove_dir_all(root).unwrap();
}
