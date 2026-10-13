use super::*;

#[test]
fn the_data_home_shadows_the_system_entries() {
    let scratch = Scratch::new();
    scratch.write(
        "home/applications/editor.desktop",
        application("My Editor", "editor --mine", "").as_bytes(),
    );
    scratch.write(
        "home/applications/clock.desktop",
        application("Clock", "clock", "Hidden=true").as_bytes(),
    );
    scratch.write(
        "system/applications/editor.desktop",
        application("Editor", "editor", "").as_bytes(),
    );
    scratch.write(
        "system/applications/clock.desktop",
        application("Clock", "clock", "").as_bytes(),
    );
    scratch.write(
        "system/applications/kde/konsole.desktop",
        application("Konsole", "konsole", "").as_bytes(),
    );
    scratch.write(
        "system/applications/browser.desktop",
        application("browser", "browser", "").as_bytes(),
    );
    scratch.write("system/applications/notes.txt", b"not an entry");
    let programs = environment(&scratch, &["home", "system"]).programs();
    assert_eq!(
        ids(&programs),
        ["browser.desktop", "kde-konsole.desktop", "editor.desktop"],
        "sorted by name, the hidden clock shadows the system one, and a subfolder is part of the id"
    );
    assert_eq!(programs[2].name, "My Editor");
    assert_eq!(programs[2].exec, "editor --mine");
}
