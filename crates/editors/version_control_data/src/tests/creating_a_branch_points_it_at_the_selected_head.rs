use super::*;

#[test]
fn creating_a_branch_points_it_at_the_selected_head() {
    let (mut editor, block) = editor();

    editor.click("repository.new-branch-name");
    editor.run();
    editor.text("topic");
    editor.run();
    editor.click("repository.create-branch");
    editor.run();

    let data = block.read().unwrap();
    assert_eq!(data.branch_head("topic"), data.branch_head("main"));
    drop(data);
    editor.snapshot("creating_a_branch_points_it_at_the_selected_head");
}
