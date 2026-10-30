use super::*;
use crate::native::Incoming;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_menu_from_a_zoomed_out_window_escapes_at_full_size() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let chosen = create_signal(Vec::<usize>::new());
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <ForEach keys={(0..4).collect::<Vec<usize>>()}>|index: usize| {
                let label = format!("Item {index}");
                view! {
                    <unstyled::MenuItem
                        label
                        on_click={move || chosen.update(|chosen| chosen.push(index))}
                    />
                }
            }</ForEach>
        };
        [view! {
            <ContextMenu items>
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let region = region.get();
    let mut harness = Harness::sized(document, vec2(160.0, 80.0));
    give_layer_space(&mut harness);
    let pos = harness.rect(region).max - vec2(10.0, 10.0);
    press_secondary(&mut harness, pos);
    let full = harness.document().escaped_layers()[0].rect;
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    let zoomed = crate::LayerSpace {
        to_host: crate::HostTransform {
            origin: pos2(200.0, 100.0),
            scale: Vec2::splat(0.5),
        },
        ..LAYER_SPACE
    };
    harness
        .context()
        .set_layer_space(WindowId::MAIN, Some(zoomed));
    harness.frame(Vec::new());
    press_secondary(&mut harness, pos);

    let layer = harness.document().escaped_layers()[0].clone();
    assert!(
        (layer.rect.width() - full.width()).abs() < 1.0
            && (layer.rect.height() - full.height()).abs() < 1.0,
        "a menu from a window shown at half size is drawn at full size: {:?} against {full:?}",
        layer.rect
    );
    let pressed = zoomed.to_host.point(pos);
    assert!(
        (layer.rect.left() - pressed.x).abs() <= 2.0 && (layer.rect.top() - pressed.y).abs() <= 2.0,
        "and still opens at the press: {:?} at {pressed:?}",
        layer.rect
    );

    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(2);
    let at = zoomed.to_host.point(harness.center(row));
    let row_host = zoomed.to_host.rect(harness.rect(row));
    assert!(
        layer.rect.contains_rect(row_host),
        "the row's window rect maps through to_host into the layer: {row_host:?}"
    );
    let (id, to_host) = harness
        .document()
        .escaped_layer_of(row)
        .expect("the row is in an escaped layer");
    assert_eq!((id, to_host), (layer.id, zoomed.to_host));
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
        harness.runner().push(Incoming::layer(layer.id, input));
        harness.frame(Vec::new());
    }
    assert_eq!(
        chosen.get_untracked(),
        vec![2],
        "a press on the full-size row chooses it"
    );
}
