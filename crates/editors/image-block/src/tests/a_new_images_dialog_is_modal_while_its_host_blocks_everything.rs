use block_editor_beui::beui::accesskit::Role;
use block_editor_beui::beui::vec2;
use block_editor_beui::{Creation, DialogFrame};

use super::*;

#[test]
fn a_new_images_dialog_is_modal_while_its_host_blocks_everything() {
    let mut dialog = BeuiTest::<ImageApp>::creation(Creation::new(EditorHost::default()));
    dialog.set_size(vec2(360.0, 200.0));
    let modal = |dialog: &BeuiTest<ImageApp>| {
        let document = dialog.document();
        let view = document
            .accessibility_view()
            .expect("the dialog is described");
        let found: Vec<bool> = view
            .nodes()
            .filter(|node| node.role() == Role::Dialog)
            .map(|node| node.is_modal())
            .collect();
        found
    };

    dialog.set_dialog(DialogFrame {
        modal: true,
        ..DialogFrame::default()
    });
    assert_eq!(
        modal(&dialog),
        vec![true],
        "nothing in the plugin blocks around its own dialog, so it is modal as its host says"
    );

    dialog.set_dialog(DialogFrame::default());
    assert_eq!(
        modal(&dialog),
        vec![false],
        "a dialog its host shows over only part of itself is not modal"
    );
}
