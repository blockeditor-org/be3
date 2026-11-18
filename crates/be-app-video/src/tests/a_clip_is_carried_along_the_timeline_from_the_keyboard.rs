use be_sdk_beui::beui::{Key, Modifiers, accesskit};

use super::*;

/// What the carry's live region says.
fn said(fixture: &Fixture) -> Option<String> {
    let document = fixture.editor.document();
    document
        .accessibility_view()?
        .nodes()
        .find(|node| node.role() == accesskit::Role::Status && node.label().is_some())
        .and_then(|node| node.label().map(str::to_owned))
}

/// The timeline is a `custom` site, its places a function of the point; from the keyboard a
/// clip steps along a grid of frames on its lanes, naming each stop, and lands where it stops.
#[test]
fn a_clip_is_carried_along_the_timeline_from_the_keyboard() {
    let mut fixture = Fixture::new();
    let clips = [fixture.insert(0), fixture.insert(1)];
    fixture.settle();
    // Zoomed in, so the clips run past what the timeline shows.
    for _ in 0..4 {
        fixture.editor.click("video.zoom_in");
    }
    fixture.settle();
    let first = format!("video.clip.{}", clips[0]);
    let timeline = fixture.editor.rect_of("video.timeline");
    let second_at = fixture.editor.rect_of(&format!("video.clip.{}", clips[1]));
    assert!(
        second_at.right() > timeline.right(),
        "the second clip runs past the timeline's edge"
    );

    let mut reached = false;
    for _ in 0..40 {
        fixture.editor.key(Key::Tab, Modifiers::NONE);
        let document = fixture.editor.document();
        if document.focused_node().is_some()
            && document.focused_node() == document.find_test_id(&first)
        {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches a clip");
    let label = fixture.editor.accessible_label(&first).unwrap_or_default();
    assert!(
        label.starts_with("Clip ") && label.ends_with(", lane 1, 0:00.00"),
        "named for its block and where it starts, not {label:?}"
    );
    fixture.editor.snapshot("a_clip_focused_from_the_keyboard");

    fixture.editor.key(Key::Space, Modifiers::NONE);
    fixture.editor.key(Key::ArrowRight, Modifiers::NONE);
    let stepped = said(&fixture).unwrap_or_default();
    assert!(
        stepped.starts_with("Over lane 1, "),
        "a step names the stop it reached, not {stepped:?}"
    );
    for _ in 0..400 {
        fixture.editor.key(Key::ArrowRight, Modifiers::NONE);
    }
    let marker = fixture.editor.rect_of("video.drop_marker");
    assert!(
        timeline.contains_rect(marker),
        "the timeline follows the carry: its marker at {marker:?} is shown in {timeline:?}"
    );
    assert_ne!(
        fixture.editor.rect_of(&format!("video.clip.{}", clips[1])),
        second_at,
        "panned to show it"
    );
    fixture.editor.key(Key::Enter, Modifiers::NONE);

    let order = fixture
        .video()
        .clips()
        .iter()
        .map(|clip| clip.id)
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [clips[1], clips[0]],
        "carried past the other clip, it lands after it"
    );
}
