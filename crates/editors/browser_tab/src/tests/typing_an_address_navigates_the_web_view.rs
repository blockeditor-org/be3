use super::*;

#[test]
fn typing_an_address_navigates_the_web_view() {
    let mut tab = Harness::new();
    let _ = tab.editor.take_web_view_commands();

    tab.editor.click("browser.address");
    tab.run();
    tab.editor.key_press_modifiers(Modifiers::CTRL, Key::A);
    tab.run();
    tab.editor.text("example.org");
    tab.run();
    tab.editor.click("browser.go");
    tab.run();
    tab.run();

    assert_eq!(
        tab.urls().last().map(String::as_str),
        Some("https://example.org")
    );
    assert!(
        tab.editor
            .take_web_view_commands()
            .contains(&WebViewCommand::Load("https://example.org".into()))
    );
    tab.editor
        .snapshot("typing_an_address_navigates_the_web_view");
}
