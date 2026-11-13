use super::*;

/// Back from a document opened from the files goes to the files, every time. The tapped files row
/// had the keyboard, so the stacked dock hands it to the document it shows in the files' place;
/// left in the hidden files, it went back to them when the back slide built them behind the
/// document, which showed the files there instead and sent back to the document.
#[test]
fn on_a_phone_back_leaves_each_document_opened_from_the_files() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Hello");
    host.wait("Hello");
    host.run(&["resize", "390x844"]);
    host.click("#dock.back");
    host.pause(600);
    host.gone("Pane \"Text\"");

    for _ in 0..2 {
        host.run(&["tap", "TreeItem \"Hello\""]);
        host.wait("Pane \"Text\"");
        host.run(&["back"]);
        host.pause(600);
        host.gone("Pane \"Text\"");
        host.wait("Pane \"Files\"");
        host.gone("#dock.back");
    }

    host.run(&["tap", "TreeItem \"Hello\""]);
    host.wait("Pane \"Text\"");
    host.run(&["back", "left"]);
    host.run(&["back", "0.5"]);
    host.pause(16);
    let (document_x, _, _, _) = host.place("Pane \"Text\"");
    let (files_x, _, _, _) = host.place("Pane \"Files\"");
    assert!(
        document_x > 0.0 && files_x < document_x,
        "the document slides off to the right over the files it goes back to: the document at \
         {document_x}, the files at {files_x}"
    );
    assert!(
        host.shown("Button \"Rename\""),
        "the page sliding away is the document under its own bar, not a copy of the files"
    );
    host.run(&["back", "commit"]);
    host.pause(600);
    host.gone("Pane \"Text\"");
    host.gone("#dock.back");
    host.close();
}
