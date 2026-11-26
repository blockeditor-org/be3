use super::*;

#[test]
fn a_windows_app_id_finds_its_desktop_entry() {
    let scratch = Scratch::new();
    scratch.write(
        "home/applications/com.github.maoschanz.drawing.desktop",
        application("Drawing", "drawing", "").as_bytes(),
    );
    scratch.write(
        "system/applications/com.github.maoschanz.drawing.desktop",
        application("Old Drawing", "drawing", "").as_bytes(),
    );
    let entries = environment(&scratch, &["home", "system"]).entries();
    assert_eq!(
        entries
            .of_window("com.github.maoschanz.drawing")
            .map(|entry| entry.name.as_str()),
        Some("Drawing"),
        "the first data folder that has the entry names the program"
    );
    assert!(entries.of_window("unknown").is_none());
    assert!(entries.of_window("").is_none());
    assert!(entries.of_window("../drawing").is_none());
}
