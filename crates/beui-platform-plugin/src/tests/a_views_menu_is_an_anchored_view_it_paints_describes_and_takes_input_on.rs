use beui_core::accesskit::Role;
use beui_core::input::PointerButton;
use block_editor_plugin::{HostTransform, AnchorSpace, TimedInput, ViewAnchor};

use super::*;

const ORIGIN: Pos2 = Pos2::new(200.0, 100.0);

fn spaced() -> ShownView {
    let mut region = whole(100.0, 60.0);
    region.describe = Some(1);
    region.anchor_space = Some(AnchorSpace {
        to_host: HostTransform {
            origin: ORIGIN,
            scale: Vec2::splat(1.0),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
    });
    region
}

fn row(frame: &Frame, label: &str) -> Option<Rect> {
    let tree = frame.description.as_ref()?.tree.as_ref()?;
    let node = |id| {
        tree.nodes
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, node)| node)
    };
    let parent = |id| {
        tree.nodes
            .iter()
            .find(|(_, node)| node.children().contains(&id))
            .map(|(parent, _)| *parent)
    };
    let &(mut id, ref found) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::MenuItem && node.label() == Some(label))?;
    let mut bounds = found.bounds()?;
    loop {
        if let Some(transform) = node(id).and_then(|node| node.transform()) {
            bounds = transform.transform_rect_bbox(bounds);
        }
        match parent(id) {
            Some(up) => id = up,
            None => break,
        }
    }
    Some(Rect::from_min_max(
        pos2(bounds.x0 as f32, bounds.y0 as f32),
        pos2(bounds.x1 as f32, bounds.y1 as f32),
    ))
}

#[test]
fn a_views_menu_is_an_anchored_view_it_paints_describes_and_takes_input_on() {
    let _root = RootScope::new();
    let open_at = create_signal(None::<Pos2>);
    let chosen = create_signal(Vec::<usize>::new());
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            let items = view! {
                <ForEach keys={(0..4).collect::<Vec<usize>>()}>|index: usize| {
                    let label = format!("Item {index}");
                    view! {
                        <beui::unstyled::MenuItem
                            label
                            on_click={move || chosen.update(|chosen| chosen.push(index))}
                        />
                    }
                }</ForEach>
            };
            view! {
                <styled::ContextMenu
                    items
                    open_at={Prop::dynamic(move || open_at.get())}
                    on_close={move || open_at.set(None)}
                >
                    <styled::Frame />
                </styled::ContextMenu>
            }
        }),
    );
    let shown = spaced();
    one(&mut surface, &shown);
    assert!(one(&mut surface, &shown).anchored.is_empty());

    open_at.set(Some(pos2(90.0, 50.0)));
    let frame = one(&mut surface, &shown);
    let [layer] = frame.anchored.as_slice() else {
        panic!("the open menu is one layer: {:?}", frame.anchored);
    };
    assert_eq!(layer.placement.parent, ViewId::MAIN);
    assert!(
        matches!(layer.placement.anchor, ViewAnchor::Rect(_)),
        "a context menu is anchored to its press"
    );
    let region = Rect::from_min_size(ORIGIN, vec2(100.0, 60.0));
    assert!(
        !region.contains_rect(layer.placement.rect),
        "the menu reaches past its region's frame: {:?}",
        layer.placement.rect
    );
    let menu = anchored_shown(&shown, layer);
    let framed = one_anchored(&mut surface, &menu);
    assert!(
        framed.changed,
        "the view a frame anchors asks to be painted"
    );
    assert!(
        surface.painting(menu.id()).is_some(),
        "the anchored view has a painting of its own"
    );
    assert!(
        row(&frame, "Item 2").is_none(),
        "the menu's rows are not in the region's description"
    );
    let item = row(&framed, "Item 2").expect("the menu's rows are in its own view's description");
    assert!(
        framed
            .floating
            .iter()
            .any(|floating| floating.contains(item.center())),
        "the row is described where the view takes input, in its own points: {item:?} in {:?}",
        framed.floating
    );
    let at = item.center();
    for input in [
        Input::PointerMoved(at),
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
        },
    ] {
        surface.input(&menu, &TimedInput::from(input), None);
        one(&mut surface, &shown);
    }
    assert_eq!(
        chosen.get_untracked(),
        vec![2],
        "a press on the layer's row chooses it"
    );
    assert!(
        one(&mut surface, &shown).anchored.is_empty(),
        "and closes the menu"
    );

    open_at.set(Some(pos2(90.0, 50.0)));
    let opened = one(&mut surface, &shown).anchored;
    assert_eq!(opened.len(), 1);
    surface.dismiss_anchored(&shown, &[opened[0].view]);
    assert!(
        one(&mut surface, &shown).anchored.is_empty(),
        "a dismissal from the host closes the layer"
    );
    assert_eq!(open_at.get_untracked(), None);
}
