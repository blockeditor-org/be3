use super::*;

#[test]
fn a_windows_class_finds_the_entry_that_names_it() {
    let scratch = Scratch::new();
    scratch.write(
        "share/applications/org.codeberg.dnkl.foot.desktop",
        application("Foot", "foot", "Icon=foot\nStartupWMClass=foot\n").as_bytes(),
    );
    scratch.write(
        "share/applications/debian-xterm.desktop",
        application(
            "XTerm",
            "xterm",
            "Icon=mini.xterm\nStartupWMClass=XTerm\nNoDisplay=true\n",
        )
        .as_bytes(),
    );
    scratch.write(
        "share/applications/kate.desktop",
        application("Kate", "kate", "Icon=kate\n").as_bytes(),
    );
    let environment = environment(&scratch, &["share"]);
    let icon = |program_id: &str| {
        environment
            .entry_of_window(program_id)
            .and_then(|entry| entry.icon)
    };
    assert_eq!(
        icon("foot").as_deref(),
        Some("foot"),
        "an app id that is no entry's name finds the entry whose window class it is"
    );
    assert_eq!(
        icon("XTerm").as_deref(),
        Some("mini.xterm"),
        "an X11 window's class finds its entry, hidden or not, in any case"
    );
    assert_eq!(
        icon("Kate").as_deref(),
        Some("kate"),
        "a class that is an entry's name in another case finds it"
    );
    assert!(environment.entry_of_window("").is_none());
    assert!(environment.entry_of_window("unknown").is_none());
}
