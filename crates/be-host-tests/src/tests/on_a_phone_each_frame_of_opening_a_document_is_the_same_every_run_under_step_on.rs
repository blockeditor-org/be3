use super::*;

/// Frames recorded after the tap: the opening takes three, and two more show it has settled.
const FRAMES: usize = 5;

/// On a phone, with presents late and the clock stopped, each frame of opening a document from
/// the files is recorded as it is: the document's own bar over its view not yet drawn, then the
/// view drawn. Under `step on` no frame runs between commands, so every run records the same
/// frames; without it about a run in five records others (guides/running/drive.md, The clock).
#[test]
fn on_a_phone_each_frame_of_opening_a_document_is_the_same_every_run_under_step_on() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.run(&["resize", "390x844"]);
    host.click("#dock.back");
    host.pause(600);
    host.gone("Pane \"Text\"");
    host.wait("TreeItem \"Text\"");
    host.run(&["present", "late"]);
    host.run(&["step", "on"]);

    host.run(&["--no-settle", "tap", "TreeItem \"Text\""]);
    for _ in 0..FRAMES {
        host.record_frame();
    }
    host.run(&["step", "off"]);
    host.run(&["present", "now"]);
    host.wait("Pane \"Text\"");
    host.snapshot("on_a_phone_opening_a_document_frame_by_frame");
    host.close();
}
