use super::*;

/// Frames the opening may take, with presents late, before the host settles.
const FRAMES: usize = 16;

/// On a phone, a document opened from the files takes the files' place in the stacked dock.
/// With each app's present taken in only at the next frame (`present late`), a frame is drawn
/// between an app's layout and its present; with the clock stopped and `step on`, each drive
/// command steps one frame. Each frame's tree and state are read: an app's present is held for
/// the next frame, and no frame lists the document and the files at once. Which frame ends the
/// loop varies with load, so a recording of the frames is a test of its own
/// (`on_a_phone_each_frame_of_opening_a_document_is_the_same_every_run_under_step_on`).
#[test]
fn on_a_phone_opening_a_document_from_the_files_steps_frame_by_frame_with_presents_late() {
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
    let mut held = false;
    let mut opened = false;
    for _ in 0..FRAMES {
        let state = host.run(&["state"]);
        let holding = state.contains("has a present held for the next frame");
        held |= holding;
        let tree = host.tree();
        assert!(
            !(tree.contains("Pane \"Text\"") && tree.contains("TreeItem \"Text\"")),
            "a frame shows the document and the files at once:\n{tree}"
        );
        // Until the document shows with no present held; what follows until the host settles
        // waits on the server and the block store's work for it, which `snapshot` waits out.
        if tree.contains("Pane \"Text\"") && !holding {
            opened = true;
            break;
        }
    }
    assert!(held, "no frame held an app's present for the next one");
    assert!(opened, "the opening took more than {FRAMES} frames");
    host.run(&["step", "off"]);
    host.run(&["present", "now"]);
    host.wait("Pane \"Text\"");
    host.close();
}
