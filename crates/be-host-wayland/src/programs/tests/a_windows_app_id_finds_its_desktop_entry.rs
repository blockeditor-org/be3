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
    let environment = environment(&scratch, &["home", "system"]);
    assert_eq!(
        environment
            .entry("com.github.maoschanz.drawing")
            .map(|entry| entry.name),
        Some("Drawing".to_owned()),
        "the first data folder that has the entry names the program"
    );
    assert!(environment.entry("unknown").is_none());
    assert!(environment.entry("").is_none());
    assert!(environment.entry("../drawing").is_none());
}
