use super::editor;
use block_ui_test::prelude::*;

#[test]
fn the_copy_button_copies_a_code_block_without_its_fences() {
    let mut editor = editor(concat!(
        "Before\n",
        "\n",
        "- A list\n",
        "  ```rust\n",
        "  fn main() {\n",
        "      println!(\"hi\");\n",
        "  }\n",
        "  ```\n",
        "\n",
        "> ```\n",
        "> quoted\n",
        "> ```\n",
    ));
    editor.run();
    editor.snapshot("the_copy_button_copies_a_code_block_without_its_fences");

    editor.click("text.copy_code.3");
    editor.click("text.copy_code.9");

    assert_eq!(
        editor.take_copied_text(),
        vec![
            "fn main() {\n    println!(\"hi\");\n}".to_owned(),
            "quoted".to_owned(),
        ]
    );
}
