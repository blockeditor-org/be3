use super::*;

#[test]
fn values_can_only_be_changed_after_the_warning_is_dismissed() {
    let mut test = editor();
    assert!(test.shown("block-debug.unlock"), "the warning is shown");
    test.double_click(format!("{COUNT}.row").as_str());
    test.run();
    assert!(!test.shown(&rename_field()), "nothing opens for editing");
    test.snapshot("editing_the_data_starts_behind_a_warning");
}
