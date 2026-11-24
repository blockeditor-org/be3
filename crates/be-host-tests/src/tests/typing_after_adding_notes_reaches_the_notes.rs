use super::*;

#[test]
fn typing_after_adding_notes_reaches_the_notes() {
    // Add notes places the notes block's editor asking for the keyboard, which moves there at
    // the commit that places it: once the editor is up, what is typed lands in the notes. Keys
    // typed before its app runs reach nothing (decisions/keys_are_never_held.md), so this types
    // once the host settled.
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("#infinite-canvas.dock");
    host.click("#presentation.notes.add");
    host.run(&["settle"]);
    host.type_text("Thank the team");
    host.wait("Thank the team");
    host.run(&["settle"]);
    host.snapshot("typing_after_adding_notes_reaches_the_notes");
    host.close();
}
