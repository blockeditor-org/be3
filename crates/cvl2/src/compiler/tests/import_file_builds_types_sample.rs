use super::*;

#[test]
fn import_file_builds_types_sample() {
    let artifact = import_file("types.qxc", include_str!("../../../samples/types.qxc"))
        .unwrap_or_else(|errors| panic!("types.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let files: Vec<(&str, String)> = folder
        .value
        .iter()
        .map(|(name, file)| {
            let ComptimeValueBuildArtifact::File(file) = file else {
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
        [
            ("walk.txt", "walk".to_string()),
            ("stroll.txt", "stroll".to_string()),
        ]
    );
}
