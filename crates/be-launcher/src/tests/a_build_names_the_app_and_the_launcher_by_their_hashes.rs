use super::*;

#[test]
fn a_build_names_the_app_and_the_launcher_by_their_hashes() {
    let described = json!({
        "commit": "abc",
        "app": {"hash": "aa", "size": 3},
        "launcher": {"hash": "bb", "size": 2},
    })
    .to_string();

    let build = Build::parse(described.as_bytes()).unwrap();

    assert_eq!(build.commit, "abc");
    assert_eq!(build.app.hash, "aa");
    assert_eq!(build.launcher.size, 2);
    assert!(Build::parse(b"{\"commit\": \"abc\"}").is_err());
    assert_eq!(
        Slot::PullRequest(12).manifest_url("abc"),
        "https://be3-ci.b-cdn.net/android/pr-12/build.json?commit=abc"
    );
    assert_eq!(
        Slot::Main.object_url("aa"),
        "https://be3-ci.b-cdn.net/android/main/aa.apk"
    );
}
