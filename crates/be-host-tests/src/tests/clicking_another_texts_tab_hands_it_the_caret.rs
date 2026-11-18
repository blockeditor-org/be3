use super::*;

/// As with Ctrl+Tab, pressing the tab of another text while one has the keyboard hands it, caret
/// and all, to the text that tab shows, both ways round.
#[test]
fn clicking_another_texts_tab_hands_it_the_caret() {
    let mut host = HostTest::start(Options::default());
    for name in ["One", "Two"] {
        start_adding_to_my_files(&mut host, "Text");
        host.key("ctrl+a");
        host.type_text(name);
        host.key("Enter");
        host.wait(&format!("TreeItem \"{name}\""));
    }
    host.wait("Tab \"Two\" selected");
    host.click("Tab \"One\"");
    host.click("#text.surface");
    host.type_text("one");
    host.wait("MultilineTextInput value=\"one\" focused");
    host.click("Tab \"Two\"");
    host.click("#text.surface");
    host.type_text("two");
    host.wait("MultilineTextInput value=\"two\" focused");

    host.click("Tab \"One\"");
    host.wait("MultilineTextInput value=\"one\" focused");
    host.type_text("!");
    host.wait("MultilineTextInput value=\"one!\" focused");
    host.click("Tab \"Two\"");
    host.wait("MultilineTextInput value=\"two\" focused");
    host.type_text("?");
    host.wait("MultilineTextInput value=\"two?\" focused");
    host.close();
}
