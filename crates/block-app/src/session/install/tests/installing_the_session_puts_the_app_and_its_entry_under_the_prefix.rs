use super::*;

#[test]
fn installing_the_session_puts_the_app_and_its_entry_under_the_prefix() {
    let root = std::env::temp_dir().join(format!("block-app-install-{}", uuid::Uuid::new_v4()));
    let source = root.join("app");
    let prefix = root.join("prefix");
    std::fs::create_dir_all(source.join("data/notes/files")).unwrap();
    std::fs::write(source.join(EXECUTABLE), "the app").unwrap();
    std::fs::write(source.join("notes.plugin.json"), "{}").unwrap();
    std::fs::write(source.join("data/notes/files/readme.txt"), "hello").unwrap();
    let stale = prefix.join("lib/block-app/stale.plugin.json");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(
        prefix.join("lib/block-app").join(EXECUTABLE),
        "an older app",
    )
    .unwrap();
    std::fs::write(&stale, "{}").unwrap();

    let installed = place(&source, &prefix).unwrap();
    place(&source, &prefix).unwrap();

    let home = prefix.join("lib/block-app");
    assert_eq!(installed.home, home);
    assert_eq!(
        std::fs::read_to_string(home.join(EXECUTABLE)).unwrap(),
        "the app"
    );
    assert!(home.join("notes.plugin.json").is_file());
    assert_eq!(
        std::fs::read_to_string(home.join("data/notes/files/readme.txt")).unwrap(),
        "hello"
    );
    assert!(
        !stale.exists(),
        "a plugin left from an older install is gone"
    );
    assert_eq!(
        std::fs::read_link(prefix.join("bin/block-app")).unwrap(),
        home.join(EXECUTABLE)
    );
    assert_eq!(
        std::fs::read_to_string(prefix.join("share/wayland-sessions/block-app.desktop")).unwrap(),
        session_entry(&home.join(EXECUTABLE))
    );
    std::fs::remove_dir_all(root).unwrap();
}
