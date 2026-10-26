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

    let [wide, narrow] = harness.document().children(tabs)[..] else {
        panic!("tabs hold their tab list and the select they collapse into");
    };
    assert!(harness.document().node_rect(wide).is_some());
    assert!(
        harness.document().node_rect(narrow).is_none(),
        "tabs that fit lay out only the tab list"
    );
    assert_eq!(
        harness.document().api::<styled::ChoiceApi>(wide).selected(),
        Some(1)
    );

    harness.resize(vec2(120.0, VIEWPORT.y));
    harness.frame(Vec::new());

    assert!(
        harness.document().node_rect(wide).is_none(),
        "tabs too wide for their container leave the tab list unplaced"
    );
    assert!(harness.document().node_rect(narrow).is_some());
    assert_eq!(
        harness
            .document()
            .api::<styled::SelectApi>(narrow)
            .selected(),
        Some(1)
    );
}
