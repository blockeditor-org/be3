use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tabs_collapse_into_a_select_when_their_container_is_narrow() {
    let _root = crate::reactive::RootScope::new();
    let tabs = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Tabs
                @node_ref=&tabs
                options={view! {
                    <unstyled::ChoiceOption value="List" label="List" />
                    <unstyled::ChoiceOption value="Load" label="Load" />
                    <unstyled::ChoiceOption value="Name" label="Name" />
                }}
                selected="Load"
            />
        }
    });
    let tabs = tabs.get();

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let shown = |harness: &Harness| shown_fit(harness.document(), tabs);
    let wide = shown(&harness);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(wide)
            .selected(),
        Some("Load"),
        "tabs that fit show the tab list"
    );

    harness.resize(vec2(120.0, VIEWPORT.y));
    harness.frame(Vec::new());

    assert!(
        harness.document().node_rect(wide).is_none(),
        "tabs too wide for their container leave the tab list unplaced"
    );
    let narrow = shown(&harness);
    assert!(harness.document().node_rect(narrow).is_some());
    assert_eq!(
        harness
            .document()
            .api::<styled::SelectApi<&str>>(narrow)
            .selected(),
        Some("Load")
    );
}
