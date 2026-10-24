use super::*;

#[test]
fn pdfium_repositories_are_kept_for_what_the_libraries_use() {
    let libraries = json!({
        "core": {"srcs": ["core/a.cpp"], "include_dirs": ["third_party/abseil-cpp"]},
        "icu": {"srcs": ["third_party/icu/source/common/uchar.cpp"], "include_dirs": []},
    });
    let libraries = libraries.as_object().unwrap();

    let used: Vec<&str> = [
        "build",
        "buildtools",
        "third_party/abseil-cpp",
        "third_party/icu",
        "third_party/ic",
        "third_party/zlib",
    ]
    .into_iter()
    .filter(|path| crate::pdfium::used(libraries, path))
    .collect();

    assert_eq!(used, ["build", "third_party/abseil-cpp", "third_party/icu"]);
}
