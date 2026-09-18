use super::*;

#[test]
fn selecting_a_branch_shows_its_history() {
    let (mut editor, block) = editor();
    let head = block.read().unwrap().branch_head("main").cloned().unwrap();
    block.operate(
        block_client::blocks::version_control_data::VersionControlDataOperation::SetBranch {
            name: "topic".to_owned(),
            expected: None,
            commit: head,
        },
    );
    editor.run();

    assert_eq!(editor.label("repository.history"), "History  (main)");

    editor.click("repository.branch.topic");
    editor.run();

    assert_eq!(editor.label("repository.history"), "History  (topic)");
}
