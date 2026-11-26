use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Forwarded, ForwardedInput};
use beui::layer::{OverlayAnchor, Placement};

/// A menu open elsewhere holds the pointer from a catcher it does not cover: the catcher hears
/// the pointer leave, is not hovered while the menu is open whatever input comes, and once it
/// closes hears where the still pointer is, so an app's hover comes back (a dock bar dropped
/// under the pointer shows its button's tooltip every time).
#[test]
fn a_forwarding_catcher_under_a_still_pointer_hears_where_it_is_once_a_menu_closes() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<Forwarded>::new());
    let hovered = create_signal(Vec::<bool>::new());
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    on_forward={move |input: ForwardedInput| {
                        hovered.update(|hovered| hovered.push(input.hovered));
                        heard.update(|heard| heard.extend(input.events))
                    }}
                >
                    <Frame width=100.0 height=100.0 />
                </Interactive>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(200.0, 200.0))
                    placement=Placement::At
                    open={open}
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(pos2(50.0, 50.0))]);

    open.set(true);
    harness.frame(Vec::new());
    hovered.update(|hovered| hovered.clear());
    harness.frame(vec![Input::Modifiers(Modifiers::default())]);
    assert!(
        hovered.with_untracked(|hovered| hovered.iter().all(|hovered| !hovered)),
        "the menu keeps the catcher beneath it from being hovered"
    );

    open.set(false);
    harness.frame(Vec::new());
    harness.frame(vec![Input::Modifiers(Modifiers::default())]);
    let last = heard.with_untracked(|heard| heard.last().cloned());
    assert!(
        matches!(last, Some(Forwarded::Pointer { .. })),
        "the catcher last heard where the pointer is, not {last:?}"
    );
}
