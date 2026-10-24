use super::*;

#[test]
fn pdfium_libraries_are_the_compiled_targets_under_the_root() {
    let project = json!({"targets": {
        "//:pdfium": {"deps": ["//core/fxcrt:fxcrt", "//third_party:headers"]},
        "//core/fxcrt:fxcrt": {
            "deps": ["//third_party/zlib:zlib"],
            "sources": ["//core/fxcrt/a.cpp", "//core/fxcrt/a.h"],
            "defines": ["NDEBUG", "USE_UDEV"],
            "include_dirs": ["//", "//out/gen/", "//third_party/abseil-cpp/"],
            "cflags_c": ["-std=c11", "-Wall"],
            "cflags_cc": ["-std=c++20", "-fno-rtti", "-Wall"],
        },
        "//third_party/zlib:zlib": {
            "sources": ["//third_party/zlib/inflate.c"],
            "cflags_cc": ["-frtti"],
        },
        "//third_party:headers": {"sources": ["//third_party/only.h"]},
        "//unreachable:unreachable": {"sources": ["//unreachable/b.cpp"]},
    }});

    let libraries = crate::pdfium::libraries(&project).unwrap();

    assert_eq!(
        Value::Object(libraries),
        json!({
            "core_fxcrt_fxcrt": {
                "label": "//core/fxcrt:fxcrt",
                "srcs": ["core/fxcrt/a.cpp"],
                "defines": ["NDEBUG"],
                "include_dirs": ["", "third_party/abseil-cpp"],
                "c_flags": ["-std=c11"],
                "cxx_flags": ["-std=c++20"],
            },
            "third_party_zlib_zlib": {
                "label": "//third_party/zlib:zlib",
                "srcs": ["third_party/zlib/inflate.c"],
                "defines": [],
                "include_dirs": [],
                "c_flags": [],
                "cxx_flags": ["-frtti"],
            },
        })
    );
}
