#[test]
fn pdfium_repositories_download_as_github_archives() {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let archive = |url| crate::pdfium::archive(url, commit).unwrap();

    assert_eq!(
        archive("https://pdfium.googlesource.com/pdfium"),
        [
            format!("https://github.com/chromium/pdfium/archive/{commit}.tar.gz"),
            format!("pdfium-{commit}"),
        ]
    );
    assert_eq!(
        archive("https://chromium.googlesource.com/external/github.com/jk-jeon/dragonbox.git"),
        [
            format!("https://github.com/jk-jeon/dragonbox/archive/{commit}.tar.gz"),
            format!("dragonbox-{commit}"),
        ]
    );
    assert_eq!(
        archive("https://chromium.googlesource.com/chromium/deps/icu.git"),
        [
            format!(
                "https://github.com/chromium-full-mirror/chromium_deps_icu/archive/{commit}.tar.gz"
            ),
            format!("chromium_deps_icu-{commit}"),
        ]
    );
    assert!(crate::pdfium::archive("https://example.com/a.git", commit).is_err());
}
