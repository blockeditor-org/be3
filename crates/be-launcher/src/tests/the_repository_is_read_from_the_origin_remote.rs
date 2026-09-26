use super::*;

#[test]
fn the_repository_is_read_from_the_origin_remote() {
    for remote in [
        "https://github.com/blockeditor-org/be3.git",
        "https://github.com/blockeditor-org/be3",
        "git@github.com:blockeditor-org/be3.git\n",
        "http://proxy@127.0.0.1:4000/git/blockeditor-org/be3",
    ] {
        assert_eq!(parse_remote(remote), Some(repository()), "{remote}");
    }
    assert_eq!(parse_remote("be3"), None);
}
