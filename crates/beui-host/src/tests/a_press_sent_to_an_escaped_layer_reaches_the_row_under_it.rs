use super::*;
use crate::native::Incoming;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_press_sent_to_an_escaped_layer_reaches_the_row_under_it() {
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

    let layer = harness.document().escaped_layers()[0].clone();
    let content = harness
        .document()
        .api::<unstyled::ContextMenuApi>(menu)
        .menu();
    let row = harness
        .document()
        .api::<unstyled::MenuListApi>(content)
        .row_button(2);
    let at = LAYER_SPACE.to_host.point(harness.center(row));
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
        "a press at a host position on the layer chooses the row drawn there"
    );
}
