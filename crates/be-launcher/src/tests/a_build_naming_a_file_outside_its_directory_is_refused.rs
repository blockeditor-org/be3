use super::*;

#[test]
fn a_build_naming_a_file_outside_its_directory_is_refused() {
    let described = |path: &str| {
        json!({
            "commit": "abc",
            "shell": "shell",
            "launcher": {"hash": "00", "size": 1},
            "files": [{"path": path, "hash": "00", "size": 1}],
        })
        .to_string()
    };

    let build = Build::parse(described("assets/plugins.json").as_bytes()).unwrap();
    assert_eq!(build.files[0].path, "assets/plugins.json");
    assert_eq!(build.size(), 1);
    for path in ["../escape", "/etc/passwd", "assets/../../escape", ""] {
        assert!(Build::parse(described(path).as_bytes()).is_err(), "{path}");
    }
    assert_eq!(
        Slot::PullRequest(12).manifest_url("abc"),
        "https://be3-ci.b-cdn.net/android/pr-12/build.json?commit=abc"
    );
}
