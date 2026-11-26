use super::*;

/// The text app is held back (`drive hold`) while Ctrl+Shift+Tab shows the other text, so the
/// host gives that text's dock tab view the keyboard before the app has opened a window for it:
/// once the app catches up, its caret is in that text and typing lands there.
#[test]
fn a_text_shown_while_its_app_is_slow_takes_the_caret_once_it_catches_up() {
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

    host.run(&["hold", "be3.text"]);
    host.key("ctrl+shift+Tab");
    host.wait("Tab \"One\" selected");
    host.run(&["hold", "off"]);
    // The frames it sends catching up wait on the stopped clock, as drive says when this is left out.
    host.pause(100);
    host.wait("MultilineTextInput value=\"one\" focused");
    host.type_text("!");
    host.wait("MultilineTextInput value=\"one!\" focused");
    host.close();
}
