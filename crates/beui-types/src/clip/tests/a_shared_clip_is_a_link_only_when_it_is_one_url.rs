use super::*;

#[test]
fn a_shared_clip_is_a_link_only_when_it_is_one_url() {
    assert_eq!(
        shared_link(" https://be3.example/#invite/CODE\n"),
        Some("https://be3.example/#invite/CODE")
    );
    assert_eq!(shared_link("HTTP://be3.example"), Some("HTTP://be3.example"));
    assert_eq!(shared_link("join me: https://be3.example"), None);
    assert_eq!(shared_link("https://be3.example and more"), None);
    assert_eq!(shared_link("mailto:ada@be3.example"), None);
    assert_eq!(shared_link(""), None);
}
