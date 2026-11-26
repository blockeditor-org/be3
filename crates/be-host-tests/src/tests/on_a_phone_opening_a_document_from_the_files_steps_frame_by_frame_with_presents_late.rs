use super::*;

/// Frames the opening may take, with presents late, before the host settles.
const FRAMES: usize = 16;

/// On a phone, a document opened from the files takes the files' place in the stacked dock.
/// With each app's present taken in only at the next frame (`present late`), a frame is drawn
/// between an app's layout and its present; each drive command steps one. Each frame's tree
/// and state are read: an app's present is held for the next frame, and no frame lists the
/// document and the files at once. What the frame between paints is not recorded yet: whether
/// it shows the files' old bar over the document's undrawn view depends on when the root's
/// present lands (plans/backlog.md, "A stacked dock's old bar over the child that replaced it").
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

    host.run(&["--no-settle", "tap", "TreeItem \"Text\""]);
    let mut held = false;
    let mut settled = false;
    for _ in 0..FRAMES {
        let state = host.run(&["state"]);
        held |= state.contains("has a present held for the next frame");
        let tree = host.tree();
        assert!(
            !(tree.contains("Pane \"Text\"") && tree.contains("TreeItem \"Text\"")),
            "a frame shows the document and the files at once:\n{tree}"
        );
        if tree.contains("Pane \"Text\"") && state.lines().any(|line| line == "settled") {
            settled = true;
            break;
        }
    }
    assert!(held, "no frame held an app's present for the next one");
    assert!(settled, "the opening took more than {FRAMES} frames");
    host.run(&["present", "now"]);
    host.wait("Pane \"Text\"");
    host.close();
}
