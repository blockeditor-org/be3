use super::*;
use beui::styled::{Frame, List, Size, Space};
use beui_macros::view;

#[test]
fn a_list_leaves_no_gap_for_a_hidden_frame() {
    let document = build(|| {
        view! {
            <Root>
                <List spacing=Space::Md>
                    <Frame @test_id="first" height=Size::Sm />
                    <Show condition=false>
                        <Frame height=Size::Sm />
                    </Show>
                    <Frame @test_id="last" height=Size::Sm />
                </List>
            </Root>
        }
    });
    let test = DocumentTest::new(document, Vec2::new(200.0, 200.0));

    assert_eq!(
        test.rect_of("last").top() - test.rect_of("first").bottom(),
        Space::Md.px(),
        "a frame a Show hides is out of the tree, so it takes no room and adds no spacing"
    );
}
