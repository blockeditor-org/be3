use super::*;

#[test]
fn a_new_image_asks_for_its_file_in_the_image_editors_own_dialog() {
    let mut app = AppTest::start();
    open_add_to_my_files(&mut app);
    choose_type(&mut app, "Image");
    app.wait("#creation.confirm");
    app.run(&["pause", "600"]);
    let (_, actions_y, _, actions_height) = app.place("#creation.actions");
    let (_, card_y, _, card_height) = app.place("#picker.child");
    let below = card_y + card_height - (actions_y + actions_height);
    assert!(
        below <= 30.0,
        "the card ends just below its actions, with no blank strip: {below} below them"
    );
    app.snapshot("a_new_image_asks_for_its_file_in_the_image_editors_own_dialog");

    app.key("Escape");
    app.gone("#creation.confirm");

    open_add_to_my_files(&mut app);
    choose_type(&mut app, "Image");
    app.click("#creation.cancel");
    app.gone("#creation.confirm");
    app.close();
}
