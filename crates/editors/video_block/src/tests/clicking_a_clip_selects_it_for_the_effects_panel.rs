use super::*;

#[test]
fn clicking_a_clip_selects_it_for_the_effects_panel() {
    let (mut fixture, clip) = editor_with_clip();

    assert_eq!(fixture.video().clips().len(), 1);
    assert!(fixture.editor.shown(&format!("video.clip.{clip}")));

    fixture.editor.click(&format!("video.clip.{clip}"));
    fixture.editor.run();
    fixture.editor.run();

    assert!(
        fixture.editor.shown("video.clip-length"),
        "the effects panel shows the length of the clip that was chosen"
    );
}
