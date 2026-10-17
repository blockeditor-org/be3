use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_narrow_inspector_covering_the_app_unplaces_its_embeds() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let document = build(move || {
        view! {
            <Embed slot={slot} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(slot.placement().is_some(), "the embed is placed");

    harness.toggle_inspector();
    harness.frame(Vec::new());
    assert!(!harness.inspector().state().app_visible());
    assert!(
        slot.placement().is_none(),
        "an embed under an inspector that fills the window claims no place"
    );

    harness.toggle_picking();
    harness.frame(Vec::new());
    assert!(harness.inspector().state().app_visible());
    assert!(
        slot.placement().is_some(),
        "the embed is placed again once the app shows"
    );
}
