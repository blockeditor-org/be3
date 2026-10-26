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
                    <unstyled::ChoiceOption label="List" />
                    <unstyled::ChoiceOption label="Load" />
                    <unstyled::ChoiceOption label="Name" />
                }}
                selected=1
            />
        }
    });
    let tabs = tabs.get();

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let shown = |harness: &Harness| {
        let [portal] = harness.document().children(tabs)[..] else {
            panic!("tabs show one of the tab list and the select they collapse into");
        };
        harness.document().children(portal)[0]
    };
    let wide = shown(&harness);
    assert_eq!(
        harness.document().api::<styled::ChoiceApi>(wide).selected(),
        Some(1),
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
            .api::<styled::SelectApi>(narrow)
            .selected(),
        Some(1)
    );
}
