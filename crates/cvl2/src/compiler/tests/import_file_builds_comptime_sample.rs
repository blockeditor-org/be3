use super::*;

#[test]
fn import_file_builds_comptime_sample() {
    let artifact = import_file(
        "comptime.qxc",
        include_str!("../../../samples/comptime.qxc"),
    )
    .unwrap_or_else(|errors| panic!("comptime.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let files: Vec<(&str, String)> = folder
        .value
        .iter()
        .map(|(name, artifact)| {
            let ComptimeValueBuildArtifact::File(file) = artifact else {
                panic!("expected {name} to be a file");
            };
            (
                name.as_str(),
                String::from_utf8(file.value.clone()).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        files,
        vec![
            ("lib.c", "int f(int _a0);\nstatic int cvl2_fn_0(int _a0);\nstatic int cvl2_fn_1(int _a0);\n\nint f(int _a0) {\n    int _0 = cvl2_fn_0(_a0);\n    int _1 = cvl2_fn_1(_a0);\n    int _2 = _0 + _1;\n    int _3 = cvl2_fn_0(1);\n    int _4 = _2 + _3;\n    return _4;\n}\n\nstatic int cvl2_fn_0(int _a0) {\n    int _0 = _a0 * 2;\n    return _0;\n}\n\nstatic int cvl2_fn_1(int _a0) {\n    int _0 = _a0 * 3;\n    return _0;\n}\n".to_string()),
            ("words.txt", "ababab--120".to_string()),
        ]
    );
}
