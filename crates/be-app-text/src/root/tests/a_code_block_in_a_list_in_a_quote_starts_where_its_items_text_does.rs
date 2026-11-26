use super::*;

/// A code block in a list item in a quote lays the list's nesting after the quote's gutter, its
/// background starting where the item's text does as a code block in a list outside a quote
/// does, its lines' continuation spaces unseen; the caret after them stands at the code.
#[test]
fn a_code_block_in_a_list_in_a_quote_starts_where_its_items_text_does() {
    let mut editor = editor(
        "> - Quoted item\n>   ```\n>   let quoted = 1;\n>   ```\n\n- Item\n\n  ```\n  let listed = 1;\n  ```\n",
    );
    editor.click("text.surface");
    editor.key(Key::Home, Modifiers::CTRL);
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.key(Key::End, Modifiers::NONE);
    editor.key(Key::Home, Modifiers::NONE);
    for _ in 0..4 {
        editor.key(Key::ArrowRight, Modifiers::NONE);
    }
    editor.snapshot("a_code_block_in_a_list_in_a_quote_starts_where_its_items_text_does");
}
