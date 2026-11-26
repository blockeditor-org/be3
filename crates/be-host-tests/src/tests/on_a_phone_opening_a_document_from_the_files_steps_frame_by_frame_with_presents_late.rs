use super::*;

/// Frames the opening takes, with presents late, until the host settles.
const FRAMES: usize = 8;

/// On a phone, a document opened from the files takes the files' place in the stacked dock.
/// With each app's present taken in only at the next frame (`present late`), a frame is drawn
/// between an app's layout and its present. The first opening records every frame, as each
/// drive command steps one; the second, after back, reads each frame's tree and state, and
/// checks no frame lists the document and the files at once. The recording's second frame
/// still shows the files' bar over the document's undrawn view (plans/backlog.md, "A stacked
/// dock's old bar over the child that replaced it").
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

    host.record_frame();
    host.run(&["--no-settle", "tap", "TreeItem \"Text\""]);
    for _ in 0..FRAMES {
        host.record_frame();
    }
    let state = host.run(&["state"]);
    assert!(
        state.lines().any(|line| line == "settled"),
        "the opening took more than {FRAMES} frames:\n{state}"
    );
    host.wait("Pane \"Text\"");
    host.snapshot("on_a_phone_opening_a_document_from_the_files_steps_frame_by_frame_with_presents_late");

    host.run(&["back"]);
    host.pause(600);
    host.gone("Pane \"Text\"");
    host.wait("TreeItem \"Text\"");
    host.run(&["--no-settle", "tap", "TreeItem \"Text\""]);
    let mut held = false;
    for _ in 0..2 * FRAMES {
        let state = host.run(&["state"]);
        held |= state.contains("has a present held for the next frame");
        let tree = host.tree();
        assert!(
            !(tree.contains("Pane \"Text\"") && tree.contains("TreeItem \"Text\"")),
            "a frame shows the document and the files at once:\n{tree}"
        );
        if tree.contains("Pane \"Text\"") && state.lines().any(|line| line == "settled") {
            break;
        }
    }
    assert!(held, "no frame held an app's present for the next one");
    host.run(&["present", "now"]);
    host.wait("Pane \"Text\"");
    host.close();
}
