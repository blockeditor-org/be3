use super::*;

#[test]
fn a_direct_editor_is_placed_live_until_the_host_calls_it_preview_only() {
    assert!(!shows_preview(None, false));
    assert!(!shows_preview(Some(DirectEditorInteraction::Live), false));
    assert!(!shows_preview(
        Some(DirectEditorInteraction::Playback),
        false
    ));
    assert!(shows_preview(Some(DirectEditorInteraction::Preview), false));
    assert!(!shows_preview(Some(DirectEditorInteraction::Preview), true));
}
