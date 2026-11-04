use super::*;

#[test]
fn a_timeline_lists_the_description_reviews_commits_and_events() {
    let pull_request = PullRequest {
        number: 3,
        title: "Change".to_owned(),
        state: State::Open,
        author: Person {
            login: "ada".to_owned(),
            avatar: String::new(),
        },
        branch: "ada/change".to_owned(),
        head_sha: "abc".to_owned(),
        same_repository: true,
        labels: Vec::new(),
        created: 0,
        updated: 0,
        url: "https://github.com/blockeditor-org/be3/pull/3".to_owned(),
        body: String::new(),
    };
    let events = [
        json!({ "event": "committed", "sha": "0123456789", "message": "fix: it\n\nbody" }),
        json!({ "event": "reviewed", "id": 7, "state": "changes_requested", "user": { "login": "grace" }, "body": "", "submitted_at": "1970-01-01T00:00:10Z" }),
        json!({ "event": "reviewed", "id": 8, "state": "commented", "user": { "login": "grace" }, "body": "" }),
        json!({ "event": "labeled", "actor": { "login": "ada" }, "label": { "name": "bug" } }),
        json!({ "event": "head_ref_deleted", "actor": { "login": "ada" } }),
        json!({ "event": "subscribed", "actor": { "login": "ada" } }),
    ];
    let review_comments = [json!({
        "pull_request_review_id": 7,
        "user": { "login": "grace" },
        "path": "src/lib.rs",
        "line": 4,
        "body": "Why?"
    })];
    let entries = parse_timeline(&pull_request, &events, &review_comments);
    assert_eq!(entries.len(), 5);
    let Entry::Comment { verb, body, .. } = &entries[0] else {
        panic!("the description comes first");
    };
    assert_eq!(verb, "opened this pull request");
    assert_eq!(
        body,
        &vec![Block::Paragraph("No description provided.".to_owned())]
    );
    assert_eq!(
        entries[1],
        Entry::Commit {
            sha: "0123456".to_owned(),
            message: "fix: it".to_owned(),
        }
    );
    let Entry::Comment {
        tone, inline, when, ..
    } = &entries[2]
    else {
        panic!("the review follows the commit");
    };
    assert_eq!((*tone, *when), (Tone::ChangesRequested, 10));
    assert_eq!(inline[0].location, "src/lib.rs:4");
    assert_eq!(inline[0].body, vec![Block::Paragraph("Why?".to_owned())]);
    assert_eq!(
        entries[3],
        Entry::Event {
            actor: "ada".to_owned(),
            text: "added the bug label".to_owned(),
            when: 0,
        }
    );
    assert!(branch_deleted(&entries));
}
