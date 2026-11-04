use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROW: f32 = 20.0;

#[test]
fn the_edge_two_rows_share_hovers_only_the_lower_one() {
    let _root = crate::reactive::RootScope::new();
    let upper = create_signal(false);
    let lower = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive on_hover_change={upper}>
                    <Frame height=ROW />
                </Interactive>
                <Interactive on_hover_change={lower}>
                    <Frame height=ROW />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Input::PointerMoved(pos2(10.0, ROW - 0.5))]);
    assert!(upper.get_untracked() && !lower.get_untracked());

    harness.frame(vec![Input::PointerMoved(pos2(10.0, ROW))]);
    assert!(
        !upper.get_untracked() && lower.get_untracked(),
        "a pointer on the edge between two rows hovers one of them"
    );
}
