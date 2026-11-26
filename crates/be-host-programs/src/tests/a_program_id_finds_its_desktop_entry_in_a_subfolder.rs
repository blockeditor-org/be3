use super::*;

#[test]
fn a_program_id_finds_its_desktop_entry_in_a_subfolder() {
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
    let entries = environment(&scratch, &["data"]).entries();
    let named = |program_id: &str| {
        entries
            .of_window(program_id)
            .map(|entry| entry.name.clone())
    };
    assert_eq!(
        named("kde-org.kde-kate"),
        Some("Kate".to_owned()),
        "a '-' in a desktop file id stands for a subfolder"
    );
    assert_eq!(
        named("wine-Programs-Notepad++"),
        Some("Notepad++".to_owned()),
        "however deep the subfolder"
    );
    assert_eq!(
        entries
            .of_window("kde-org.kde-kate")
            .map(|entry| entry.id.as_str()),
        Some("kde-org.kde-kate.desktop"),
        "the entry's id joins its subfolders with '-'"
    );
    assert!(named("kde-").is_none() && named("-kate").is_none());
    assert!(
        named("..-outside").is_none(),
        "no id reaches out of the folder"
    );
}
