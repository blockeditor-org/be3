use super::*;
use std::collections::HashSet;

use beui_core::display::{Display, Layer, Part};
use beui_core::node::NodeId;

#[test]
fn the_app_and_the_inspector_paint_displays_a_renderer_can_tell_apart() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.toggle_inspector();
    let output = harness.frame(Vec::new());

    let mut seen = Vec::new();
    collect_layers(&output.main().layers, &mut seen);
    let documents: HashSet<u64> = seen.iter().map(|(document, _)| *document).collect();
    assert!(
        documents.len() >= 2,
        "the app and the inspector painted from {} documents",
        documents.len()
    );
    let mut keys = HashSet::new();
    for key in &seen {
        assert!(keys.insert(*key), "two displays were painted as {key:?}");
    }
}

fn collect_layers(layers: &[Layer], seen: &mut Vec<(u64, NodeId)>) {
    for layer in layers {
        match layer {
            Layer::Shape(_) => {}
            Layer::Display(_, display) => collect(display, seen),
            Layer::Group(_, inner) => collect_layers(inner, seen),
        }
    }
}

fn collect(display: &Display, seen: &mut Vec<(u64, NodeId)>) {
    seen.push((display.document, display.node));
    collect_parts(&display.parts, seen);
}

fn collect_parts(parts: &[Part], seen: &mut Vec<(u64, NodeId)>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(_, _, child) => collect(child, seen),
            Part::Group(_, inner) => collect_parts(inner, seen),
        }
    }
}
