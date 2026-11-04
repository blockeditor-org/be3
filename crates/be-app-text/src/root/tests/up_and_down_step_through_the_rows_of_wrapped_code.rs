use be_type_text::TextLanguage;
use uuid::Uuid;

use super::{BeuiTest, TextBlock, step_through_wrapped_line};

#[test]
fn up_and_down_step_through_the_rows_of_wrapped_code() {
    let first = "fn main() {\n";
    let long = "    let message = \"a string literal long enough to wrap onto several rows of the code view, so the arrows step through it a row at a time instead of crossing it at once, and it goes on past the edge again and again, row after row, for as long as it takes to fill three or four of them at this width\";\n";
    let mut editor = BeuiTest::<crate::root::TextRoot>::editing(Uuid::new_v4());
    editor.hold(
        None,
        TextBlock::new(format!("{first}{long}}}\n"))
            .with_language(TextLanguage::Rust)
            .content(),
    );
    editor.run();
    step_through_wrapped_line(&mut editor, 3, first.len()..first.len() + long.len() - 1);

    editor.snapshot("up_and_down_step_through_the_rows_of_wrapped_code");
}
