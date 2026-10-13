use super::*;

#[test]
fn import_file_builds_reflect_sample() {
    let artifact = import_file("reflect.qxc", include_str!("../../../samples/reflect.qxc"))
        .unwrap_or_else(|errors| panic!("reflect.qxc failed to build: {errors:?}"));

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
            ("clamp.c", "int clamp(int p_0, int p_1) {\n  int v_0 = p_0;\n  int v_1 = p_1;\n  int v_2;\n  {\n  int v_3 = v_0 > v_1;\n  if (v_3) {\n  v_2 = v_1;\n  goto v_2_end;\n  }\n  int v_4 = v_1 * 2;\n  int v_5 = v_0 + v_4;\n  v_2 = v_5;\n  }\n  v_2_end:;\n  int v_6 = v_2;\n  return v_6;\n}\n".to_string()),
            ("poly.c", "long poly(long p_0) {\n  long v_0 = p_0;\n  long v_1 = v_0 * v_0;\n  long v_2 = 3;\n  long v_3 = v_2 * v_0;\n  long v_4 = v_1 + v_3;\n  long v_5 = 1;\n  long v_6 = v_4 + v_5;\n  return v_6;\n}\n".to_string()),
        ]
    );
}
