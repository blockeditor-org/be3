use super::*;

#[test]
fn an_app_id_finds_its_desktop_entry_in_a_subfolder() {
    let scratch = Scratch::new();
    scratch.write(
        "data/applications/kde/org.kde-kate.desktop",
        application("Kate", "kate", "").as_bytes(),
    );
    scratch.write(
        "data/applications/wine/Programs/Notepad++.desktop",
        application("Notepad++", "notepad", "").as_bytes(),
    );
    scratch.write(
        "data/outside.desktop",
        application("Outside", "outside", "").as_bytes(),
    );
    let environment = environment(&scratch, &["data"]);
    let named = |app_id: &str| environment.entry(app_id).map(|entry| entry.name);
    assert_eq!(
        named("kde-org.kde-kate"),
        Some("Kate".to_owned()),
        "a '-' in a desktop file id may stand for a subfolder, and another may not"
    );
    assert_eq!(
        named("wine-Programs-Notepad++"),
        Some("Notepad++".to_owned()),
        "however deep the subfolder"
    );
    assert_eq!(
        environment.entry("kde-org.kde-kate").map(|entry| entry.id),
        Some("kde-org.kde-kate.desktop".to_owned()),
        "the entry keeps the id it was asked by"
    );
    assert!(named("kde-").is_none() && named("-kate").is_none());
    assert!(
        named("..-outside").is_none(),
        "no id reaches out of the folder"
    );
}
