use super::*;

use crate::root::be_command;

fn be(sha256: &str) -> Vec<(String, String)> {
    vec![("be".to_owned(), sha256.to_owned())]
}

#[test]
fn a_download_of_be_must_be_plain_https_with_a_sha256() {
    assert!(be_command("tree", Uuid::nil(), Some(DEPLOYED_BE), &be(BE_SHA256)).is_ok());
    assert!(be_command("tree", Uuid::nil(), None, &be(BE_SHA256)).is_ok());
    for url in [
        "http://be3.example/be",
        "https://be3.example/be'; rm -rf ~; '",
        "https://be3.example/be\\",
    ] {
        assert!(
            be_command("tree", Uuid::nil(), Some(url), &be(BE_SHA256)).is_err(),
            "{url}"
        );
    }
    for hash in [
        "",
        "abc",
        &BE_SHA256.replacen('9', "z", 1),
        &format!("{BE_SHA256}0"),
    ] {
        assert!(
            be_command("tree", Uuid::nil(), Some(DEPLOYED_BE), &be(hash)).is_err(),
            "{hash}"
        );
    }
    let mut beside = be(BE_SHA256);
    beside.push(("terminal_types.wasm".to_owned(), BE_SHA256.to_owned()));
    assert!(be_command("tree", Uuid::nil(), None, &beside).is_ok());
    for name in ["", ".hidden", "../be", "a/b", "it's"] {
        let mut named = be(BE_SHA256);
        named.push((name.to_owned(), BE_SHA256.to_owned()));
        assert!(
            be_command("tree", Uuid::nil(), None, &named).is_err(),
            "{name:?} is no plain file name"
        );
    }
    let no_be = vec![("terminal_types.wasm".to_owned(), BE_SHA256.to_owned())];
    assert!(be_command("tree", Uuid::nil(), None, &no_be).is_err());
}
