use super::*;
use block_editor_beui::be_block::be_model::RichText;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use text_editor_core::Value;
use text_editor_core::schema::basic::{self, BULLET, CHECK, CHECKED, LIST, NUMBER, QUOTE};

#[test]
fn a_text_label_draws_its_lists_and_quotes() {
    let text = "Groceries\nApples\nPears, and a note long enough to wrap under its bullet\nFirst\nSecond\nMilk\nEggs\nBuy what is in season.\n";
    let line = |words: &str| {
        let start = text.find(words).unwrap_or(0);
        start..start + words.len()
    };
    let mut label = entity(Uuid::from_u128(1));
    label.shape = CanvasShape::Text;
    label.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(260.0, 340.0), 0.0);
    label.text_style = CanvasTextStyle {
        wrap: true,
        ..CanvasTextStyle::default()
    };
    label.text = RichText::new(
        text,
        vec![
            basic::block(
                line("Apples").start..line("bullet").end,
                LIST,
                Value::text(BULLET),
            ),
            basic::block(
                line("First").start..line("Second").end,
                LIST,
                Value::text(NUMBER),
            ),
            basic::block(
                line("Milk").start..line("Eggs").end,
                LIST,
                Value::text(CHECK),
            ),
            basic::block(line("Milk"), CHECKED, Value::Bool(true)),
            basic::block(line("Buy what"), QUOTE, Value::Bool(true)),
        ],
    );
    let mut editor = editor(&[label]);
    editor.run();

    editor.snapshot("a_text_label_draws_its_lists_and_quotes");
}
