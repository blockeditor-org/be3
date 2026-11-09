use super::*;

use crate::root::be_command;

#[test]
fn a_download_of_be_must_be_plain_https_with_a_sha256() {
    assert!(be_command("tree", Some(DEPLOYED_BE), BE_SHA256).is_ok());
    assert!(be_command("tree", None, BE_SHA256).is_ok());
    for url in [
        "http://be3.example/be",
        "https://be3.example/be'; rm -rf ~; '",
        "https://be3.example/be\\",
    ] {
        assert!(be_command("tree", Some(url), BE_SHA256).is_err(), "{url}");
    }
    for hash in ["", "abc", &BE_SHA256.replacen('9', "z", 1), &format!("{BE_SHA256}0")] {
        assert!(be_command("tree", Some(DEPLOYED_BE), hash).is_err(), "{hash}");
    }
}
