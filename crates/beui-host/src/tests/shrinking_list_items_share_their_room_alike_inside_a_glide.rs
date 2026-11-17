use super::*;
use crate::reactive::*;
use crate::unstyled::{Glide, Scroll};

/// Two items that shrink, each a scroll that fits its content as far as there is room, in a
/// row too narrow for both; laid out with and without a `Glide` around each.
fn widths(glided: bool) -> [f32; 2] {
    let _root = RootScope::new();
    let items = [NodeRef::new(), NodeRef::new()];
    let document = build_node(move || {
        let item = |node: NodeRef, width: f32| {
            let content = view! {
                <Scroll @node_ref=&node direction=Direction::Horizontal fit=true style={plain_scroll()}>
                    <ListItem shrink=1.0>
                        <Frame width=width height=20.0 />
                    </ListItem>
                </Scroll>
            };
            match glided {
                true => view! {
                    <Glide spring={Spring { stiffness: 300.0, damping: 30.0 }}>{content}</Glide>
                },
                false => content,
            }
        };
        view! {
            <Frame width=150.0 height=40.0 align=Align2::START>
                <List direction=Direction::Horizontal spacing=0.0>
                    <ListItem shrink=1.0>{item(items[0], 40.0)}</ListItem>
                    <ListItem shrink=1.0>{item(items[1], 200.0)}</ListItem>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    items.map(|item| harness.rect(item.get()).width())
}

#[test]
fn shrinking_list_items_share_their_room_alike_inside_a_glide() {
    let bare = widths(false);
    assert!(
        bare[0] + bare[1] <= 150.5 && bare[0] < 40.5,
        "the row is too narrow, so the items shrink: {bare:?}"
    );
    assert_eq!(
        widths(true),
        bare,
        "a glide around each item changes nothing of how they share the row"
    );
}
