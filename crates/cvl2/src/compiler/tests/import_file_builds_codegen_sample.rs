use super::*;

#[test]
fn import_file_builds_codegen_sample() {
    let artifact = import_file("codegen.qxc", include_str!("../../../samples/codegen.qxc"))
        .unwrap_or_else(|errors| panic!("codegen.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(name, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file");
    };
    assert_eq!(name, "sum.c");
    assert_eq!(
        String::from_utf8(file.value.clone()).unwrap(),
        "int t_0 = 0;\nint t_1 = t_0 + 1;\nint t_2 = t_1 + 2;\nint t_3 = t_2 + 3;\nreturn t_3;\n"
    );
}
