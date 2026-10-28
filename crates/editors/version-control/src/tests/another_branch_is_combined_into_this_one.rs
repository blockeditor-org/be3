use super::*;

#[test]
fn another_branch_is_combined_into_this_one() {
    let status = VersionStatus {
        branches: vec![
            VersionBranch {
                name: "draft".into(),
                head: [8; 32],
            },
            VersionBranch {
                name: "main".into(),
                head: [7; 32],
            },
        ],
        working: Some(working()),
        ..VersionStatus::default()
    };
    let state = Checkout {
        branch: "main".into(),
        ..Checkout::default()
    };
    let mut editor = checkout(state, status);

    editor.click("checkout.merge.draft");

    assert_eq!(
        editor.take_version_commands(),
        vec![(
            own(&editor),
            VersionCommand::Merge {
                branch: "draft".into()
            }
        )]
    );
    assert!(!editor.shown("checkout.merge.main"));
}
