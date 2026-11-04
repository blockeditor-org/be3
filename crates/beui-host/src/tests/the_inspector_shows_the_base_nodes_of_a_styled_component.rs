use super::*;
use crate::primitives::Text;
use crate::styled::*;

#[test]
fn the_inspector_shows_the_base_nodes_of_a_styled_component() {
    let (document, [_row]) = toolbar_of(|| {
        [view! {
            <ListRow>
                <Text color=Color32::WHITE>"Hello"</Text>
            </ListRow>
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        ["list", "  interactive", "    frame", "      text"]
    );
}
