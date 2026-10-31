use super::*;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::primitives::{Frame, Overlay};
use crate::reactive::*;
use crate::styled::*;
use crate::{Dismiss, LayerPolicy};

#[test]
fn clicking_an_open_selects_button_again_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let select = ComponentRef::<styled::SelectApi>::new();
    let document = build_node(move || {
        view! {
            <Frame>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(20.0, 20.0))
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Pass
                    traps_focus=false
                    open=true
                    on_dismiss={|_: Dismiss| {}}
                >
                    || {
                        let options = view! {
                            <unstyled::ChoiceOption label="Apple" />
                            <unstyled::ChoiceOption label="Banana" />
                        };
                        view! {
                            <Select api={select} options />
                        }
                    }
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = select.get().trigger();

    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(select.get().open(), "the first click opens the select");

    harness.click(harness.center(trigger));
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    assert!(
        !select.get().open(),
        "a second click on the button of a select floating over a layer closes it rather than closing and opening it again"
    );
}
