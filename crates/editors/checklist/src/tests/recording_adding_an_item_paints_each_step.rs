use super::*;

#[test]
fn recording_adding_an_item_paints_each_step() {
    let mut checklist = Harness::new(&[]);
    checklist.click("checklist.draft");
    checklist.type_text("buy milk");

    example_recording_a_gesture(&mut checklist.editor);
}

fn example_recording_a_gesture(editor: &mut BeuiTest<ChecklistApp>) {
    editor.record();
    editor.click("checklist.add");
    editor.run();
    editor.record();
    editor.snapshot("adding_an_item_records_each_step");
}
