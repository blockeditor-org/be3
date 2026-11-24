use super::*;

#[test]
fn a_manifest_names_its_files_by_hash_inside_the_app() {
    let hash = "a".repeat(64);
    let described = |path: &str, sha256: &str| {
        json!({
            "product": "native-linux_x86_64",
            "commit": "c".repeat(40),
            "files": [
                {"path": "be-host", "sha256": hash, "size": 3, "executable": true},
                {"path": path, "sha256": sha256, "size": 2, "executable": false},
            ],
        })
        .to_string()
    };

    let manifest = helper::parse_manifest(described("data/be3.pdf/index.json", &hash).as_bytes()).unwrap();

    assert_eq!(manifest.product, "native-linux_x86_64");
    assert!(manifest.files[0].executable);
    assert!(!manifest.files[1].executable);
    for (path, sha256) in [
        ("../outside", hash.as_str()),
        ("/etc/passwd", hash.as_str()),
        ("data//twice", hash.as_str()),
        ("plain", "not a hash"),
    ] {
        assert!(
            helper::parse_manifest(described(path, sha256).as_bytes()).is_err(),
            "{path}"
        );
    }
    assert!(helper::parse_manifest(b"{\"product\": \"android\"}").is_err());
}
