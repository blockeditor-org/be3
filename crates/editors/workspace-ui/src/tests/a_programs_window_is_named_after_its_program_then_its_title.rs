use super::*;

#[test]
fn a_programs_window_is_named_after_its_program_then_its_title() {
    let named = |app_id: &str, name: &str, title: &str| block_editor_beui::HostWindow {
        app_id: app_id.to_owned(),
        name: name.to_owned(),
        ..window(1, title, None)
    };
    let cases = [
        (
            named("org.gnome.Nautilus", "Files", "Downloads"),
            "Files \u{2014} Downloads",
        ),
        (named("org.gnome.Nautilus", "Files", "Files"), "Files"),
        (
            named(
                "org.gnome.TextEditor",
                "Text Editor",
                "notes.txt - Text Editor",
            ),
            "Text Editor \u{2014} notes.txt",
        ),
        (
            named("org.gnome.Nautilus", "", "Downloads"),
            "Nautilus \u{2014} Downloads",
        ),
        (
            named("org.gnome.Nautilus", "", "org.gnome.Nautilus"),
            "Nautilus",
        ),
        (
            named("C:\\Program Files\\Paint\\mspaint.exe", "", "Untitled"),
            "Mspaint \u{2014} Untitled",
        ),
        (
            named(
                "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
                "",
                "Calculator",
            ),
            "WindowsCalculator \u{2014} Calculator",
        ),
        (named("", "", "Untitled"), "Untitled"),
        (named("", "", ""), "Window"),
    ];
    for (window, expected) in cases {
        assert_eq!(
            block_shell::window_name(&window, None),
            expected,
            "{window:?}"
        );
    }

    let parent = named("com.github.maoschanz.drawing", "Drawing", "Untitled image");
    let dialog = named("", "", "Save changes?");
    assert_eq!(
        block_shell::window_name(&dialog, Some(&parent)),
        "Drawing \u{2014} Save changes?",
        "a dialog with no name of its own takes its parent's"
    );
    assert_eq!(block_shell::program_name(&dialog, Some(&parent)), "Drawing");
}
