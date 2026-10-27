use block_editor_beui::beui::{Key, Modifiers, vec2};
use block_editor_beui::{Creation, DialogAnswer, DialogFrame};

use super::*;

#[test]
fn a_new_image_asks_for_its_file_in_a_dialog_it_draws_and_closes() {
    let mut dialog = BeuiTest::<ImageApp>::creation(Creation::new(EditorHost::default()));
    dialog.set_size(vec2(360.0, 200.0));
    dialog.run();
    assert_eq!(
        dialog.handles_dismiss(),
        Some(true),
        "its DialogBody answers a dismissal itself"
    );
    assert_eq!(dialog.label("creation.confirm"), "Create");
    dialog.snapshot("a_new_image_asks_for_its_file_in_a_dialog_it_draws");

    dialog.click("creation.confirm");
    assert!(
        dialog.take_dialog_closes().is_empty(),
        "Create waits for a file"
    );

    dialog.click("creation.cancel");
    dialog.dismiss();
    dialog.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        dialog.take_dialog_closes(),
        vec![DialogAnswer::Dismissed; 3],
        "Cancel, the host's dismissal and Escape each ask the host to close it"
    );

    dialog.set_size(vec2(360.0, 260.0));
    dialog.set_dialog(DialogFrame {
        narrow: true,
        bottom_inset: 24.0,
    });
    dialog.snapshot("a_new_image_on_a_phone_stretches_its_buttons_above_the_inset");
}
