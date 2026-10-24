use super::*;

#[test]
fn the_terminal_shows_the_hosts_screen_and_follows_its_output() {
    let mut harness = Harness::new(true);

    harness.show(&[prompt()]);
    harness.connect_host();
    assert!(resizes(&harness.sent()) >= 1, "it tells the host its size");

    let mut listed = prompt();
    listed.push("ls", TerminalStyle::default());
    let blue = TerminalStyle {
        foreground: Some(TerminalColor {
            r: 90,
            g: 140,
            b: 255,
        }),
        bold: true,
        ..TerminalStyle::default()
    };
    let mut files = TerminalLine::styled("README.md  ", TerminalStyle::default());
    files.push("src", blue);
    let mut typing = prompt();
    typing.push("claude twice", TerminalStyle::default());
    harness.show(&[listed, files, typing]);

    assert_eq!(harness.shown("terminal.host"), "\"ada on build-box\"");
    assert_eq!(harness.shown("terminal.status"), "\"\"");
    assert!(harness.sent().is_empty(), "the screen follows the document");
    harness
        .editor
        .snapshot("the_terminal_shows_the_hosts_screen_and_follows_its_output");
}
