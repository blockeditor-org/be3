use super::*;

#[test]
fn a_desktop_entry_reads_its_localized_name_and_its_lists() {
    let text = "\
# a comment
[Desktop Entry]
Type=Application
Name=Files
Name[de]=Dateien
GenericName=File Manager
Comment=Browse\\sthe files
Keywords=folder;manager;semi\\;colon;
Keywords[de]=Ordner;
Icon=org.gnome.Nautilus
Exec=nautilus --new-window %U
Terminal=false
OnlyShowIn=GNOME;Unity;

[Desktop Action new-window]
Name=New Window
Exec=nautilus --other
";
    let english =
        DesktopEntry::parse("files.desktop", "/a/files.desktop", text, &[]).expect("it parses");
    assert_eq!(english.name, "Files");
    assert_eq!(english.generic_name, "File Manager");
    assert_eq!(english.comment, "Browse the files");
    assert_eq!(english.keywords, ["folder", "manager", "semi;colon"]);
    assert_eq!(english.icon.as_deref(), Some("org.gnome.Nautilus"));
    assert_eq!(
        english.exec, "nautilus --new-window %U",
        "an action's keys are not the entry's"
    );
    assert_eq!(english.only_show_in, ["GNOME", "Unity"]);
    assert!(!english.terminal);

    let german = DesktopEntry::parse(
        "files.desktop",
        "/a/files.desktop",
        text,
        &locales("de_DE.UTF-8"),
    )
    .expect("it parses");
    assert_eq!(german.name, "Dateien");
    assert_eq!(german.keywords, ["Ordner"]);
    assert_eq!(
        german.generic_name, "File Manager",
        "an untranslated key falls back"
    );

    assert!(
        DesktopEntry::parse("x.desktop", "/x.desktop", "[Desktop Entry]\nExec=x\n", &[]).is_none(),
        "an entry without a name is not one"
    );
    assert!(
        DesktopEntry::parse("x.desktop", "/x.desktop", "Name=x\nExec=x\n", &[]).is_none(),
        "a file without the Desktop Entry group is not one"
    );
}
