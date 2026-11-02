use super::*;
use crate::primitives::{Frame, List};
use crate::styled::Text;

#[test]
fn content_past_the_windows_edge_is_cut_off() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=300.0 />
                <Text>"below the window's foot"</Text>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(200.0, 100.0));
    harness.frame(Vec::new());

    let found: Vec<String> = harness
        .document()
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found.iter().any(|found| found.contains("cut off")),
        "{found:#?}"
    );
}
