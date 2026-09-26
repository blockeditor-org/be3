use super::*;

#[test]
fn checking_a_branch_out_asks_the_host() {
    let status = VersionStatus {
        branches: branches(),
        log: vec![VersionCommit {
            id: [7; 32],
            parents: Vec::new(),
            author: [1; 16],
            time: 1_000,
            message: "Started versioning".into(),
        }],
        ..VersionStatus::default()
    };
    let mut editor = repository(Repository::default(), status);

    editor.click("repository.checkout.main");
    editor.run();

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
