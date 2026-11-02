use super::*;

#[test]
fn switching_the_profile_changes_this_viewers_chrome_and_the_default() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle)).with_top_bar(false);
    let profile =
        |editor: &BeuiTest<CanvasApp>| editor.content::<CanvasContent>(None).root().profile;

    assert_eq!(profile(&editor), CanvasProfile::Prototyping);
    assert!(!editor.shown("infinite-canvas.selection"));
    assert!(!editor.shown("infinite-canvas.dock.tool.Frame"));
    assert!(editor.menu_entry("canvas.profile.vector").enabled);

    editor.pick_menu("canvas.profile.vector");
    editor.run();
    assert!(
        editor.shown("infinite-canvas.selection"),
        "the inspector is vector's"
    );
    assert!(editor.shown("infinite-canvas.dock.tool.Frame"));
    assert_eq!(
        profile(&editor),
        CanvasProfile::Vector,
        "the canvas reopens in the profile last used"
    );
    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    assert!(!editor.shown("infinite-canvas.style"));
    editor.snapshot("switching_the_profile_changes_this_viewers_chrome_and_the_default");

    let peer = editor
        .content::<CanvasContent>(None)
        .root()
        .set_profile(CanvasProfile::Prototyping);
    editor.edit::<CanvasContent>(None, &peer);
    editor.run();
    assert_eq!(profile(&editor), CanvasProfile::Prototyping);
    assert!(
        editor.shown("infinite-canvas.selection"),
        "a peer switching profile leaves this viewer's alone"
    );

    let reopened = vector_editor(std::slice::from_ref(&rectangle));
    assert!(reopened.shown("infinite-canvas.selection"));
}
