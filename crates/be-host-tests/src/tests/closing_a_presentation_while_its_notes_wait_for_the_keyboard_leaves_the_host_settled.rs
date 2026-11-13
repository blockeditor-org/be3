use super::*;

#[test]
fn closing_a_presentation_while_its_notes_wait_for_the_keyboard_leaves_the_host_settled() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("#infinite-canvas.dock");
    host.run(&["--no-settle", "click", "#presentation.notes.add"]);
    host.run(&["--no-settle", "type", "Thank"]);
    // The presentation's tab closes while the notes may still be loading and the keys typed wait
    // for them; nothing the notes' view does afterwards may reach what the presentation freed.
    host.run(&["--no-settle", "click", "Button \"Close tab\""]);
    host.type_text(" the team");
    host.run(&["settle"]);
    host.wait("No file open");
    host.gone("Thank");
    host.close();
}
