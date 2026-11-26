use super::*;

/// A text editor's dock joins the workspace's, so each text tab shows a group whose body is
/// built again each time it is shown. Ctrl+Tab and Ctrl+Shift+Tab between two texts that had the
/// keyboard hand it, caret and all, to the text each shows, both ways round, and typing lands
/// there.
#[test]
fn ctrl_tab_between_two_texts_hands_the_caret_to_the_text_shown() {
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

    host.key("ctrl+shift+Tab");
    host.wait("Tab \"One\" selected");
    host.wait("MultilineTextInput value=\"one\" focused");
    host.type_text("!");
    host.wait("MultilineTextInput value=\"one!\" focused");

    host.key("ctrl+Tab");
    host.wait("Tab \"Two\" selected");
    host.wait("MultilineTextInput value=\"two\" focused");
    host.type_text("?");
    host.wait("MultilineTextInput value=\"two?\" focused");

    host.key("ctrl+Tab");
    host.wait("Tab \"One\" selected");
    host.wait("MultilineTextInput value=\"one!\" focused");
    host.key("ctrl+shift+Tab");
    host.wait("Tab \"Two\" selected");
    host.wait("MultilineTextInput value=\"two?\" focused");
    host.snapshot("ctrl_tab_between_two_texts_hands_the_caret_to_the_text_shown");
    host.close();
}
