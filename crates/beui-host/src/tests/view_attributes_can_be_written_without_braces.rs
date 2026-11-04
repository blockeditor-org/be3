use super::*;
use crate::FontId;
use crate::primitives::*;
use crate::reactive::*;
use beui::Overflow;
use beui::TextAlign;

const LABEL: &str = "Unbraced";
const LABEL_SIZE: f32 = 20.0;
const BOX_WIDTH: f32 = 120.0;

#[test]
fn view_attributes_can_be_written_without_braces() {
    let _root = crate::reactive::RootScope::new();
    let box_ref = NodeRef::new();
    let label = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame
                        @node_ref=&box_ref
                        width=BOX_WIDTH
                        height=40.0
                        fill={Color32::from_gray(40)}
                        radius=4.0
                    >
                        <Text
                            @node_ref=&label
                            align=TextAlign::End
                            overflow=Overflow::Visible
                            font={FontId::proportional(LABEL_SIZE)}
                        >
                            {LABEL}
                        </Text>
                    </Frame>
                </List>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), label.get()), LABEL);
    assert_eq!(harness.rect(box_ref.get()).width(), BOX_WIDTH);
    assert_eq!(harness.rect(box_ref.get()).height(), 40.0);
}
