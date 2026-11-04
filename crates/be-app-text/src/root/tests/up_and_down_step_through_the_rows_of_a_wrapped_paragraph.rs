use super::{editor, step_through_wrapped_line};

#[test]
fn up_and_down_step_through_the_rows_of_a_wrapped_paragraph() {
    let first = "A short line.\n";
    let paragraph = "This paragraph is long enough to wrap onto several rows, so the arrows have rows to step through on their way past it rather than jumping over the whole of it at once. It keeps going for a while longer still, past the right edge again and again, until it has wrapped at least three or four times in the width of the text.\n";
    let mut editor = editor(&format!("{first}{paragraph}The end.\n"));
    step_through_wrapped_line(
        &mut editor,
        6,
        first.len()..first.len() + paragraph.len() - 1,
    );

    editor.snapshot("up_and_down_step_through_the_rows_of_a_wrapped_paragraph");
}
