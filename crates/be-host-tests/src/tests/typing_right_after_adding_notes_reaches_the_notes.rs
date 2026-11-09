use super::*;

#[test]
fn typing_right_after_adding_notes_reaches_the_notes() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("#infinite-canvas.dock");
    host.click("#presentation.notes.add");
    host.type_text("Thank the team");
    host.wait("Thank the team");
    host.run(&["settle"]);
    host.snapshot("typing_right_after_adding_notes_reaches_the_notes");
    host.close();
}
