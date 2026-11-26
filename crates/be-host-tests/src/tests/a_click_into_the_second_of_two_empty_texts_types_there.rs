use super::*;

/// Of two empty texts, the hidden one does not take the caret when the shown one is given the
/// keyboard: each reads its own views' keyboard, not the app's.
#[test]
fn a_click_into_the_second_of_two_empty_texts_types_there() {
    let mut host = HostTest::start(Options::default());
    for name in ["One", "Two"] {
        start_adding_to_my_files(&mut host, "Text");
        host.key("ctrl+a");
        host.type_text(name);
        host.key("Enter");
        host.wait(&format!("TreeItem \"{name}\""));
    }
    host.wait("Tab \"Two\" selected");
    host.click("TextInput \"Filter files\"");
    host.click("#text.surface");
    host.type_text("typed");
    host.wait("MultilineTextInput value=\"typed\" focused");
    host.click("Tab \"One\"");
    host.gone("typed");
    host.close();
}
