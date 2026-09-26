use super::*;

#[test]
fn pull_requests_are_read_from_the_github_api() {
    let listed = json!([
        {
            "number": 12,
            "title": "Add a thing",
            "state": "open",
            "draft": true,
            "merged_at": null,
            "user": { "login": "ada", "avatar_url": "https://avatars.example/ada" },
            "head": { "ref": "ada/thing", "sha": "abc", "repo": { "full_name": "blockeditor-org/be3" } },
            "labels": [{ "name": "bug", "color": "d73a4a" }],
            "created_at": "1970-01-01T00:01:00Z",
            "updated_at": "1970-01-01T00:02:00Z",
            "html_url": "https://github.com/blockeditor-org/be3/pull/12",
            "body": "Hello"
        },
        {
            "number": 9,
            "title": "Merged from a fork",
            "state": "closed",
            "draft": false,
            "merged_at": "1970-01-01T00:03:00Z",
            "user": { "login": "grace" },
            "head": { "ref": "main", "sha": "def", "repo": { "full_name": "grace/be3" } },
            "labels": [{ "name": "odd", "color": "zz" }],
            "body": null
        }
    ]);
    let pull_requests = parse_pull_requests(&listed, &repository());
    assert_eq!(pull_requests.len(), 2);
    let draft = &pull_requests[0];
    assert_eq!(draft.number, 12);
    assert_eq!(draft.state, State::Draft);
    assert_eq!(draft.author.login, "ada");
    assert_eq!(draft.branch, "ada/thing");
    assert!(draft.same_repository);
    assert_eq!(draft.labels[0].name, "bug");
    assert_eq!(draft.labels[0].color, hex_color("d73a4a"));
    assert_eq!((draft.created, draft.updated), (60, 120));
    let merged = &pull_requests[1];
    assert_eq!(merged.state, State::Merged);
    assert!(!merged.same_repository);
    assert_eq!(merged.body, "");
}
