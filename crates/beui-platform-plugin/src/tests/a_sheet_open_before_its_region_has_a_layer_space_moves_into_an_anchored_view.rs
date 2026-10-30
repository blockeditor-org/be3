use std::collections::HashMap;

use beui_core::accesskit::{Node, NodeId as AccessId, Role};
use block_editor_plugin::{HostTransform, LayerSpace, ViewAnchor};

use super::*;

#[derive(Default)]
struct Tree {
    root: Option<AccessId>,
    nodes: HashMap<AccessId, Node>,
}

impl Tree {
    fn follow(&mut self, frame: &Frame) {
        let Some(update) = frame
            .description
            .as_ref()
            .and_then(|description| description.tree.as_ref())
        else {
            return;
        };
        if let Some(tree) = &update.tree {
            self.root = Some(tree.root);
        }
        for (id, node) in &update.nodes {
            self.nodes.insert(*id, node.clone());
        }
    }

    fn buttons(&self, label: &str) -> usize {
        let mut found = 0;
        let mut queue: Vec<AccessId> = self.root.into_iter().collect();
        while let Some(id) = queue.pop() {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            if node.role() == Role::Button && node.label() == Some(label) {
                found += 1;
            }
            queue.extend(node.children().iter().copied());
        }
        found
    }
}

fn spaced(region: &Region) -> Region {
    Region {
        layers: Some(LayerSpace {
            to_host: HostTransform {
                origin: pos2(100.0, 100.0),
                scale: Vec2::splat(1.0),
            },
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
        }),
        ..region.clone()
    }
}

fn sheet_app() -> Windowed {
    Windowed::new(move |_| {
        view! {
            <styled::ModalSheet open=true fit=true on_close={|| {}}>
                <styled::Button label="Row" on_click={|| {}} />
            </styled::ModalSheet>
        }
    })
}

fn shown_spaced(surface: &mut ModuleRunner, spaced: &Region, main: &mut Tree) -> (Frame, Tree) {
    let mut frame = one(surface, spaced);
    main.follow(&frame);
    let [layer] = frame.anchored.as_slice() else {
        panic!("the open sheet is one layer: {:?}", frame.anchored);
    };
    assert!(matches!(layer.placement.anchor, ViewAnchor::Edge(_)));
    let sheet = anchored_region(spaced, layer);
    let mut own = Tree::default();
    own.follow(&one_anchored(surface, &sheet));
    for _ in 0..4 {
        frame = one(surface, spaced);
        main.follow(&frame);
        own.follow(&one_anchored(surface, &sheet));
    }
    (frame, own)
}

#[test]
fn a_sheet_open_before_its_region_has_a_layer_space_moves_into_an_anchored_view() {
    let _root = RootScope::new();
    let mut surface = launched(&EditorHost::default(), sheet_app());
    let mut first = whole(300.0, 200.0);
    first.describe = Some(1);
    let mut main = Tree::default();
    for _ in 0..4 {
        let frame = one(&mut surface, &first);
        assert!(
            frame.anchored.is_empty(),
            "without a layer space nothing escapes"
        );
        main.follow(&frame);
    }
    assert_eq!(
        main.buttons("Row"),
        1,
        "without a layer space the region describes the sheet"
    );

    let spaced = spaced(&first);
    let (frame, own) = shown_spaced(&mut surface, &spaced, &mut main);
    assert_eq!(
        main.buttons("Row"),
        0,
        "the region no longer describes the sheet it escaped from"
    );
    assert_eq!(own.buttons("Row"), 1, "the sheet's own view describes it");
    assert!(
        frame.floating.is_empty() && frame.floating_painted.is_empty(),
        "the region no longer paints the sheet: {:?}",
        frame.floating_painted
    );
}
