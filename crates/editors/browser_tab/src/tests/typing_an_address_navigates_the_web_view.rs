use super::*;

#[test]
fn typing_an_address_navigates_the_web_view() {
    let Fixture {
        mut editor,
        host,
        block,
    } = editor();
    let _ = host.take_web_view_commands();

    editor.click("browser.address");
    editor.run();
    editor.key_press_modifiers(Modifiers::CTRL, Key::A);
    editor.run();
    editor.text("example.org");
    editor.run();
    editor.click("browser.go");
    editor.run();
    editor.run();

    assert_eq!(
        urls(&block).last().map(String::as_str),
        Some("https://example.org")
    );
    assert!(
        host.take_web_view_commands()
            .contains(&WebViewCommand::Load("https://example.org".into()))
    );
    editor.snapshot("typing_an_address_navigates_the_web_view");
}
