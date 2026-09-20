use super::*;

#[test]
fn clicking_a_clip_selects_it_for_the_effects_panel() {
    let (mut editor, block, clip) = editor_with_clip();

    assert_eq!(block.read().unwrap().clips().len(), 1);
    assert!(editor.shown(&format!("video.clip.{clip}")));

    editor.click(&format!("video.clip.{clip}"));
    editor.run();
    editor.run();

    assert!(
        editor.shown("video.clip-length"),
        "the effects panel shows the length of the clip that was chosen"
    );
}
