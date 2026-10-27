use super::*;
use block_editor_beui::beui::native::overflow::OverflowKind;
use block_editor_beui::beui::{Key, Modifiers};

const MOST_STOPS: usize = 80;

#[test]
fn tabbing_through_the_dock_shows_each_focus_ring_whole() {
    for (name, mut editor) in [("wide", editor(&[card()])), ("phone", phone(&[card()]))] {
        let mut visited = Vec::new();
        let mut shown = false;
        for _ in 0..MOST_STOPS {
            editor.key(Key::Tab, Modifiers::NONE);
            let Some(focused) = editor.document().focused_node() else {
                break;
            };
            if visited.contains(&focused) {
                break;
            }
            visited.push(focused);
            let cut: Vec<String> = editor
                .document()
                .overflows()
                .into_iter()
                .filter(|overflow| overflow.kind == OverflowKind::RingCut)
                .map(|overflow| overflow.to_string())
                .collect();
            assert!(cut.is_empty(), "{name}: a focus ring is cut off: {cut:?}");
            let dock = editor
                .document()
                .find_test_id("infinite-canvas.dock.tool.Select");
            if name == "wide"
                && !shown
                && editor
                    .document()
                    .focus_ancestry()
                    .iter()
                    .any(|node| Some(*node) == dock)
            {
                shown = true;
                editor.snapshot("tabbing_through_the_dock_shows_each_focus_ring_whole");
            }
        }
        assert!(name != "wide" || shown, "the tab order reaches the dock");
    }
}
