use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn a_forwarding_catcher_in_a_menu_under_its_open_submenu_still_hears_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<ForwardedInput>::new());
    let document = build_node(move || {
        view! {
            <Frame>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open=true
                >
                    <List spacing=0.0>
                        <Interactive
                            on_forward={move |input| heard.update(|heard| heard.push(input))}
                        >
                            <Frame width=80.0 height=60.0 />
                        </Interactive>
                        <Overlay
                            anchor=OverlayAnchor::Point(pos2(200.0, 10.0))
                            placement=Placement::At
                            open=true
                        >
                            <Frame width=80.0 height=60.0 />
                        </Overlay>
                    </List>
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Input::PointerMoved(pos2(40.0, 40.0))]);
    let moves: Vec<Pos2> = heard.with_untracked(|heard| {
        heard
            .iter()
            .flat_map(|input| flattened(input.clone()))
            .filter_map(|event| match event {
                Event::PointerMoved(pos) => Some(pos),
                _ => None,
            })
            .collect()
    });
    assert_eq!(
        moves,
        [pos2(40.0, 40.0)],
        "the pointer reaches a catcher in the menu the open submenu is nested in, as a press would"
    );
}
