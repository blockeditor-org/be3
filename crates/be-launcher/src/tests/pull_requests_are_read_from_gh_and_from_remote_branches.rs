use super::*;

#[test]
fn pull_requests_are_read_from_gh_and_from_remote_branches() {
    assert_eq!(
        parse_pull_requests("feature/a\t#12 Add a thing\nno tab here\nfix-b\t#9 Fix b\n"),
        vec![
            PullRequest {
                branch: "feature/a".to_owned(),
                label: "#12 Add a thing".to_owned(),
            },
            PullRequest {
                branch: "fix-b".to_owned(),
                label: "#9 Fix b".to_owned(),
            },
        ]
    );
    assert_eq!(
        parse_branches("HEAD\nmain\n\nfeature/a\n"),
        vec![
            PullRequest {
                branch: "main".to_owned(),
                label: "main".to_owned(),
            },
            PullRequest {
                branch: "feature/a".to_owned(),
                label: "feature/a".to_owned(),
            },
        ]
    );
}
