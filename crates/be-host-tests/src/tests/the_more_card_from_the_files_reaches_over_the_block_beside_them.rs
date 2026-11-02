use super::*;

#[test]
fn the_more_card_from_the_files_reaches_over_the_block_beside_them() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.wait("Tab \"Text\"");
    open_add_to_my_files(&mut host);
    let (_, button_y, _, button_height) = host.place(NEW);
    host.click("#picker.more");
    host.wait("#picker.search");
    let (files_x, _, files_width, _) = host.place("#workspace.files");
    let (search_x, search_y, search_width, _) = host.place("#picker.search");
    assert!(
        search_y > button_y + button_height,
        "the card opens under the button that asked: {search_y} under {button_y} + {button_height}"
    );
    assert!(
        search_x + search_width > files_x + files_width,
        "the card keeps its size and reaches past the files over the block beside them: \
         {search_x} + {search_width} past {files_x} + {files_width}"
    );
    host.snapshot("the_more_card_from_the_files_reaches_over_the_block_beside_them");

    let (block_x, block_y, block_width, block_height) = host.place("#workspace.block");
    let beside = format!(
        "{},{}",
        block_x + block_width - 24.0,
        block_y + block_height - 24.0
    );
    host.run(&["click", &beside]);
    host.gone("#picker.search");
    host.close();
}
