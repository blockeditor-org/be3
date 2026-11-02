use super::*;

#[test]
fn installing_the_login_session_puts_be_host_under_the_prefix_and_its_entry_where_display_managers_look()
 {
    let root = std::env::temp_dir().join(format!("be-host-install-{}", uuid::Uuid::new_v4()));
    let source = root.join("app");
    let prefix = root.join("prefix");
    let sessions = vec![
        root.join("usr/share/wayland-sessions"),
        root.join("lightdm"),
    ];
    let old = prefix.join("share/wayland-sessions/be-host.desktop");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "an older entry").unwrap();
    std::fs::create_dir_all(source.join("data/notes/files")).unwrap();
    std::fs::write(source.join(EXECUTABLE), "be-host").unwrap();
    std::fs::write(source.join("notes.app.json"), "{}").unwrap();
    std::fs::write(source.join("data/notes/files/readme.txt"), "hello").unwrap();
    let stale = prefix.join("lib/be-host/stale.app.json");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(
        prefix.join("lib/be-host").join(EXECUTABLE),
        "an older be-host",
    )
    .unwrap();
    std::fs::write(&stale, "{}").unwrap();

    let installed = place(&source, &prefix, &sessions).unwrap();
    place(&source, &prefix, &sessions).unwrap();

    let home = prefix.join("lib/be-host");
    assert_eq!(installed.home, home);
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
    assert_eq!(
        std::fs::read_link(prefix.join("bin/be-host")).unwrap(),
        home.join(EXECUTABLE)
    );
    let entries: Vec<PathBuf> = sessions
        .iter()
        .map(|dir| dir.join("be-host.desktop"))
        .collect();
    assert_eq!(installed.entries, entries);
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
    std::fs::remove_dir_all(root).unwrap();
}
