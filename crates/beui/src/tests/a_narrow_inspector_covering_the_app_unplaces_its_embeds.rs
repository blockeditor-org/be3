use super::*;
use crate::reactive::*;

#[test]
fn a_narrow_inspector_covering_the_app_unplaces_its_embeds() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let document = build_node(move || {
        view! {
            <Embed slot={slot} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(slot.placement().is_some(), "the embed is placed");

    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.click(harness.bar_option_rect(0).center());
    harness.frame(Vec::new());
    let placed = slot.placement();
    assert!(placed.is_some());

    harness.click(harness.bar_option_rect(1).center());
    harness.frame(Vec::new());
    assert!(!harness.inspector().state().app_visible());
    assert!(
        slot.placement().is_none(),
        "an embed under an inspector that fills the window claims no place"
    );

    harness.click(harness.bar_option_rect(0).center());
    harness.frame(Vec::new());
    assert!(harness.inspector().state().app_visible());
    assert_eq!(
        slot.placement(),
        placed,
        "the embed is placed where it was once the app shows again"
    );
    assert_eq!(
        harness.document().performance().latest.work.placed,
        0,
        "showing the app again keeps the layout it had while hidden"
    );
}
