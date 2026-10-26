use super::*;

#[test]
fn a_step_is_undone_from_what_was_done() {
    let status = VersionStatus {
        branches: branches(),
        working: Some(working()),
        log: history(),
        operations: vec![
            VersionOperation {
                id: [2; 16],
                time: 2_000,
                author: [1; 16],
                description: "Saved the change \u{201c}Say more\u{201d}".into(),
                undone: false,
            },
            VersionOperation {
                id: [1; 16],
                time: 1_000,
                author: [1; 16],
                description: "Started keeping versions".into(),
                undone: false,
            },
        ],
        ..VersionStatus::default()
    };
    let state = Checkout {
        branch: "main".into(),
        ..Checkout::default()
    };
    let mut editor = checkout(state, status);
    editor.set_size(Vec2::new(800.0, 1000.0));
    editor.snapshot("a_step_is_undone_from_what_was_done");

    editor.click(format!("version.undo.{}", short_id(&[2; 16])));
    editor.run();

    assert_eq!(
        editor.take_version_commands(),
        vec![(own(&editor), VersionCommand::Undo { operation: [2; 16] })]
    );
}
