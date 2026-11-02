use std::collections::HashMap;

use be_sdk::{AnchorSpace, HostTransform};
use beui_core::accesskit::{NodeId as AccessNodeId, Role};

use super::*;

fn spaced() -> ShownView {
    let mut region = whole(200.0, 60.0);
    region.describe = Some(1);
    region.anchor_space = Some(AnchorSpace {
        to_host: HostTransform {
            origin: Pos2::new(100.0, 100.0),
            scale: Vec2::splat(1.0),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
    });
    region
}

fn focus(frame: &Frame, roles: &mut HashMap<AccessNodeId, Role>) -> Option<Role> {
    let tree = frame.description.as_ref()?.tree.as_ref()?;
    for (id, node) in &tree.nodes {
        roles.insert(*id, node.role());
    }
    roles.get(&tree.focus).copied()
}

#[test]
fn a_field_focused_in_a_views_popover_is_its_anchored_views_focus() {
    let _root = RootScope::new();
    let open = create_signal(false);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            view! {
                <styled::Popover label="Join" open={open}>|_handle: styled::PopoverHandle|
                    <styled::TextInput value={create_signal(String::new())} focused=true label="Password" />
                </styled::Popover>
            }
        }),
    );
    let shown = spaced();
    one(&mut surface, &shown);
    open.set(true);
    let frame = one(&mut surface, &shown);
    let [layer] = frame.anchored.as_slice() else {
        panic!("the open popover is one layer: {:?}", frame.anchored);
    };
    let popover = anchored_shown(&shown, layer);
    let mut roles = HashMap::new();
    for _ in 0..3 {
        let framed = one_anchored(&mut surface, &popover);
        let main = one(&mut surface, &shown);
        assert_eq!(
            focus(&framed, &mut roles),
            Some(Role::TextInput),
            "the field stays the popover's focus in frames where it does not change"
        );
        assert_ne!(
            focus(&main, &mut HashMap::new()),
            Some(Role::TextInput),
            "the view the popover escapes from does not claim its focus"
        );
    }
}
