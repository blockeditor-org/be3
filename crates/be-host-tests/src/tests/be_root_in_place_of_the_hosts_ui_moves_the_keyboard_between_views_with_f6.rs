use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_moves_the_keyboard_between_views_with_f6() {
    // be-host's own UI puts a text on a canvas; started again with the root in place of its
    // UI, with the text's view holding the keyboard, Shift+F6 moves it to the files' view and
    // F6 back to the text's, as `the_host_moves_the_keyboard_between_views_with_f6` does under
    // be-host's own UI. The palette, which lists the actions of the view holding the keyboard
    // first, shows where it is.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Text");
    let text = host.place("#text.surface");
    let text = format!("{:.0},{:.0}", text.0 + text.2 / 2.0, text.1 + text.3 / 2.0);
    let block = host.place("#infinite-canvas.dock.block");
    let block = format!("{:.0},{:.0}", block.0 + block.2 / 2.0, block.1 + block.3 / 2.0);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &text]);
    host.run(&["settle"]);
    host.key("shift+F6");
    host.run(&["settle"]);
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.snapshot("be_root_moves_the_keyboard_back_to_the_files_with_shift_f6");
    host.key("Escape");
    host.run(&["settle"]);
    host.key("F6");
    host.run(&["settle"]);
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.snapshot("be_root_moves_the_keyboard_on_to_the_text_with_f6");
    // The canvas's block picker is a popover, not a modal dialog: F6 moves the keyboard on out
    // of it, so Down no longer moves its highlight (it does without F6). A modal dialog keeps
    // the keyboard: beui cycles only the views of the topmost focus-trapping layer.
    host.key("Escape");
    host.run(&["settle"]);
    host.run(&["click", &block]);
    host.run(&["settle"]);
    host.key("F6");
    host.run(&["settle"]);
    host.key("Down");
    host.run(&["settle"]);
    host.snapshot("be_root_moves_the_keyboard_out_of_a_picker_with_f6");
    host.close();
}
