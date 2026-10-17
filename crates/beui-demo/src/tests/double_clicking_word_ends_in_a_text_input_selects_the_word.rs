use beui::reactive::*;
use beui::styled::*;
use beui_core::base::text::TextNode;

use super::*;

#[test]
fn double_clicking_word_ends_in_a_text_input_selects_the_word() {
    let document = build(|| {
        view! {
            <Frame fill=ColorRole::Background padding=Sides::all(16.0)>
                <TextInput @test_id={"test.field"} value="Weekend trip" />
            </Frame>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(420.0, 120.0));
    let input = test.document().find_test_id("test.field").unwrap();
    fn text(document: &Document, node: NodeId) -> Option<beui::NodeOf<TextNode>> {
        if let Some(text) = document.arena.kind_of::<TextNode>(node)
            && document.text(text) == "Weekend trip"
        {
            return Some(text);
        }
        document
            .children(node)
            .into_iter()
            .find_map(|child| text(document, child))
    }
    let text = text(test.document(), input).unwrap();
    for (index, offset, selection, painting) in [
        (
            12,
            20.0,
            8..12,
            "text_input_selects_the_final_word_past_its_end",
        ),
        (7, 0.0, 0..7, "text_input_selects_the_word_before_a_space"),
    ] {
        let caret = test.document().text_caret_rect(text, index, 2.0).unwrap();
        test.double_click_at(pos2(caret.left() + offset, caret.center().y));
        assert_eq!(
            beui::unstyled::text_input_selection(test.document(), input),
            vec![selection]
        );
        test.snapshot(painting);
        test.advance(std::time::Duration::from_secs(1));
    }
}
