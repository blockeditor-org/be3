use accesskit::Role;
use beui_core::app::accessibility_dump::AccessibilityDump;

use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const ROWS: u32 = 30;
const ROW_HEIGHT: f32 = 44.0;

#[test]
fn scrolling_resends_only_the_scrolled_space_and_keeps_rows_where_they_show() {
    let document = build_node(|| {
        view! {
            <List spacing=0.0>
                <Frame height=200.0>
                    <Scroll @test_id="scroll">
                        <ForEach keys={(0..ROWS).collect::<Vec<_>>()}>|row: u32|
                            <Frame height=ROW_HEIGHT>
                                <styled::Button
                                    @test_id={format!("row.{row}")}
                                    variant=styled::ButtonVariant::Primary
                                    label={format!("Row {row}")}
                                    on_click={|| {}}
                                />
                            </Frame>
                        </ForEach>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let mut dump = AccessibilityDump::new();
    dump.update(
        harness
            .frame(Vec::new())
            .accessibility_tree("Test", VIEWPORT),
    );
    let scroll = harness.find("scroll");
    harness.frame(vec![Input::PointerMoved(harness.rect(scroll).center())]);

    let output = harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -25.0)))]);
    assert_eq!(harness.document().scroll_offset(scroll), 25.0);
    let update = output.accessibility_tree("Test", VIEWPORT);
    assert!(
        update.nodes.iter().all(|(_, node)| matches!(
            node.role(),
            Role::Window | Role::ScrollView | Role::GenericContainer
        )),
        "a scroll moves the space its rows sit in and sends none of them: {:?}",
        update.nodes
    );
    dump.update(update);
    let rendered = dump.rendered();
    for row in [1, 3] {
        let id = format!("row.{row}");
        assert_eq!(
            rendered.test_ids().rect(&id),
            Some(harness.rect(harness.find(&id))),
            "{id} is reported where the scroll shows it"
        );
    }
}
