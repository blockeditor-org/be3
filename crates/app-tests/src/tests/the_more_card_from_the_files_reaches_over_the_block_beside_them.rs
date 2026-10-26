use super::*;

#[test]
fn the_more_card_from_the_files_reaches_over_the_block_beside_them() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("Tab \"Text\"");
    app.click(ADD_TO_MY_FILES);
    let (_, button_y, _, button_height) = app.place(ADD_TO_MY_FILES);
    app.click("#picker.more");
    app.wait("#picker.search");
    let (files_x, files_y, files_width, _) = app.place("#workspace.files");
    let (search_x, search_y, search_width, _) = app.place("#picker.search");
    assert!(
        search_y > button_y + button_height,
        "the card opens under the button that asked: {search_y} under {button_y} + {button_height}"
    );
    assert!(
        search_x + search_width > files_x + files_width,
        "the card keeps its size and reaches past the files over the block beside them: \
         {search_x} + {search_width} past {files_x} + {files_width}"
    );
    app.snapshot("the_more_card_from_the_files_reaches_over_the_block_beside_them");

    let (block_x, block_y, block_width, block_height) = app.place("#workspace.block");
    let beside = format!(
        "{},{}",
        block_x + block_width - 24.0,
        block_y + block_height - 24.0
    );
    app.run(&["app", "click", &beside]);
    assert!(
        app.shown("#picker.search"),
        "a press on the block beside the card leaves it open"
    );

    let above = format!("{},{}", files_x + files_width / 2.0, files_y + 8.0);
    app.run(&["app", "click", &above]);
    app.gone("#picker.search");
    app.close();
}
