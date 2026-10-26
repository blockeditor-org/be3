use block_editor_beui::DialogAnswer;
use block_editor_beui::beui::vec2;

use super::*;

#[test]
fn its_settings_dialog_offers_apply_once_the_copy_changes() {
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: Uuid::new_v4(),
            block_type: CompiledLogicContent::CONTENT_TYPE,
        },
    );
    let data = crate::app::descriptor_data(Uuid::new_v4());
    let mut settings = BeuiTest::<LogicGridApp>::settings(artifacts, data);
    settings.set_size(vec2(360.0, 200.0));
    settings.run();
    assert_eq!(settings.handles_dismiss(), Some(true));

    settings.click("artifact-settings.confirm");
    assert!(
        settings.take_dialog_closes().is_empty(),
        "Apply waits for a change"
    );

    settings.click("logic-grid.rename-with-source");
    settings.run();
    settings.snapshot("its_settings_dialog_offers_apply_once_the_copy_changes");
    settings.click("artifact-settings.confirm");
    settings.run();
    assert_eq!(settings.take_dialog_closes(), vec![DialogAnswer::Accepted]);

    settings.dismiss();
    assert_eq!(
        settings.take_dialog_closes(),
        vec![DialogAnswer::Dismissed],
        "the host's dismissal goes through Cancel"
    );
}
