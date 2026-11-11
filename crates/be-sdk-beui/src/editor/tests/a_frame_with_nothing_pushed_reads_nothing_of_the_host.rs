use super::*;

#[test]
fn a_frame_with_nothing_pushed_reads_nothing_of_the_host() {
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let _root = RootScope::new();
    let _chrome = editor.pushed(Pushed::Shows);
    // The first frames read everything once and settle the placed rect.
    editor.begin_frame();
    editor.begin_frame();

    let before = host.reads();
    for _ in 0..3 {
        editor.begin_frame();
    }
    assert_eq!(host.reads(), before, "a quiet frame reads nothing");

    host.set_chrome_shown(false);
    editor.begin_frame();
    let pushed = host.reads();
    assert!(pushed > before, "a frame after a push reads what changed");

    editor.begin_frame();
    assert_eq!(host.reads(), pushed, "and the frame after it reads nothing");
}
