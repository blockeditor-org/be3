use super::*;

#[test]
fn checking_a_branch_out_asks_the_host() {
    let status = VersionStatus {
        branches: branches(),
        log: history(),
        operations: vec![VersionOperation {
            id: [1; 16],
            time: 1_000,
            author: [1; 16],
            description: "Started keeping versions".into(),
            undone: false,
        }],
        ..VersionStatus::default()
    };
    let mut editor = repository(Repository::default(), status);

    editor.click("repository.checkout.main");

    assert_eq!(
        editor.take_version_commands(),
        vec![(
            own(&editor),
            VersionCommand::NewCheckout {
                branch: "main".into()
            }
        )]
    );
    editor.snapshot("checking_a_branch_out_asks_the_host");
}
