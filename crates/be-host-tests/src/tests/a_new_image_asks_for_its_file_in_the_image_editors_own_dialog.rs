use super::*;

#[test]
fn a_new_image_asks_for_its_file_in_the_image_editors_own_dialog() {
    let mut host = HostTest::start();
    open_add_to_my_files(&mut host);
    choose_type(&mut host, "Image");
    host.wait("#creation.confirm");
    host.run(&["pause", "600"]);
    // The card keeps the type list's size until the image editor reports its own, from its own
    // thread and after the frames a pause runs: wait until the card no longer covers a point
    // that lies 30 points below the actions, as the check below asks.
    host.run(&["gone", "#picker.child", "over", "#creation.actions+0,62"]);
    let (_, actions_y, _, actions_height) = host.place("#creation.actions");
    let (_, card_y, _, card_height) = host.place("#picker.child");
    let below = card_y + card_height - (actions_y + actions_height);
    assert!(
        below <= 30.0,
        "the card ends just below its actions, with no blank strip: {below} below them"
    );
    host.snapshot("a_new_image_asks_for_its_file_in_the_image_editors_own_dialog");

    host.key("Escape");
    host.gone("#creation.confirm");

    open_add_to_my_files(&mut host);
    choose_type(&mut host, "Image");
    host.click("#creation.cancel");
    host.gone("#creation.confirm");
    host.close();
}
