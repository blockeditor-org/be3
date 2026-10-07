use super::*;

#[test]
fn import_file_builds_reflect_sample() {
    let artifact = import_file("reflect.qxc", include_str!("../../../samples/reflect.qxc"))
        .unwrap_or_else(|errors| panic!("reflect.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(name, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file");
    };
    assert_eq!(name, "clamp.c");
    assert_eq!(
        String::from_utf8(file.value.clone()).unwrap(),
        "int clamp(int p_0, int p_1) {\n  int v_0 = p_0;\n  int v_1 = p_1;\n  int v_2;\n  {\n  int v_3 = v_0 > v_1;\n  if (v_3) {\n  v_2 = v_1;\n  goto v_2_end;\n  }\n  int v_4 = v_1 * 2;\n  int v_5 = v_0 + v_4;\n  v_2 = v_5;\n  }\n  v_2_end:;\n  int v_6 = v_2;\n  return v_6;\n}\n"
    );
}
