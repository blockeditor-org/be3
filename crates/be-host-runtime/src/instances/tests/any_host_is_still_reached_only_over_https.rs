use super::*;

#[test]
fn any_host_is_still_reached_only_over_https() {
    let hosts = vec!["*".to_owned()];

    assert!(allowed("https://api.github.com/user", &hosts));
    assert!(allowed("https://vector.openstreetmap.org/a/b.mvt", &hosts));

    assert!(!allowed("http://api.github.com/user", &hosts));
    assert!(!allowed("http://127.0.0.1:8080/", &hosts));
    assert!(!allowed("ftp://example.com/", &hosts));
    assert!(!allowed("api.github.com/user", &hosts));
}
