use super::*;

#[test]
fn checking_a_branch_out_asks_the_host() {
    let status = VersionStatus {
        branches: vec![VersionBranch {
            name: "main".into(),
            head: [7; 32],
        }],
        log: vec![VersionCommit {
            id: [7; 32],
            parents: Vec::new(),
            author: [1; 16],
            time: 1_000,
            message: "Started versioning".into(),
        }],
        ..VersionStatus::default()
    };
    let mut harness = harness(Repository::default(), status);

    harness.editor.click("repository.checkout.main");
    harness.run();

    assert_eq!(
        harness.host.take_version_commands(),
        vec![(
            harness.editor.block_id().expect("the editor has a block"),
            VersionCommand::NewCheckout {
                branch: "main".into()
            }
        )]
    );
    harness
        .editor
        .snapshot("checking_a_branch_out_asks_the_host");
}
